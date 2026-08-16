//! see more: https://agentclientprotocol.com/rfds/elicitation

use agent_client_protocol::schema::v1::{
    CreateElicitationRequest, ElicitationAcceptAction, ElicitationAction, ElicitationContentValue,
    ElicitationFormMode, ElicitationMode, ElicitationPropertySchema, ElicitationSchema,
    ElicitationScope, ElicitationSessionScope, EnumOption, MultiSelectPropertySchema,
    StringPropertySchema,
};
use agent_client_protocol::{Client, ConnectionTo};
use aries_session::session::resume_input;
use aries_tools::question::AskUserQuestionArgs;
use tracing::warn;

#[derive(Debug, Clone)]
pub enum ElicitationAnswer {
    Accepted(Vec<String>),
    Declined,
    Cancelled,
}

impl ElicitationAnswer {
    pub fn to_input(&self, question: &AskUserQuestionArgs) -> String {
        match self {
            ElicitationAnswer::Accepted(answers) => {
                resume_input(&question.question, &answers.join("; "))
            },
            ElicitationAnswer::Declined | ElicitationAnswer::Cancelled => {
                let verb = if matches!(self, ElicitationAnswer::Declined) {
                    "declined"
                } else {
                    "cancelled"
                };
                format!(
                    "The user {verb} the question \"{}\". Continue with the information you already have.",
                    question.question
                )
            },
        }
    }
}

#[derive(Debug)]
pub struct Elicitation {
    cx: ConnectionTo<Client>,
    session_id: String,
}

impl Elicitation {
    pub fn new(cx: ConnectionTo<Client>, session_id: impl Into<String>) -> Self {
        Self { cx, session_id: session_id.into() }
    }

    pub async fn ask(&self, question: &AskUserQuestionArgs) -> ElicitationAnswer {
        let property = match &question.options {
            Some(options) if !options.is_empty() && !question.custom => {
                if question.multiple {
                    let values: Vec<String> =
                        options.iter().map(|option| option.label.clone()).collect();
                    ElicitationPropertySchema::Array(MultiSelectPropertySchema::new(values))
                } else {
                    let one_of: Vec<EnumOption> = options
                        .iter()
                        .map(|option| {
                            EnumOption::new(option.label.clone(), option.label.clone())
                                .description(option.description.clone())
                        })
                        .collect();
                    ElicitationPropertySchema::String(StringPropertySchema::new().one_of(one_of))
                }
            },
            _ => ElicitationPropertySchema::String(StringPropertySchema::new()),
        };
        let schema = ElicitationSchema::new().property("answer", property, true);

        let request = CreateElicitationRequest::new(
            ElicitationMode::Form(ElicitationFormMode::new(
                ElicitationScope::Session(ElicitationSessionScope::new(self.session_id.clone())),
                schema,
            )),
            question.question.clone(),
        );

        match self.cx.send_request(request).block_task().await {
            Ok(response) => match &response.action {
                ElicitationAction::Accept(ElicitationAcceptAction { content, .. }) => {
                    let answers = match content.as_ref().and_then(|c| c.get("answer")) {
                        Some(ElicitationContentValue::StringArray(values)) => values.clone(),
                        Some(ElicitationContentValue::String(value)) => vec![value.clone()],
                        _ => Vec::new(),
                    };
                    ElicitationAnswer::Accepted(answers)
                },
                ElicitationAction::Decline => ElicitationAnswer::Declined,
                ElicitationAction::Cancel | ElicitationAction::Other(_) => {
                    ElicitationAnswer::Cancelled
                },
                _ => ElicitationAnswer::Cancelled,
            },
            Err(err) => {
                warn!("elicitation/create failed: {err}");
                ElicitationAnswer::Cancelled
            },
        }
    }
}
