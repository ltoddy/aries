use std::borrow::Cow;

use agent_client_protocol::v1::schema::{
    SessionConfigKind, SessionConfigOption, SessionConfigOptionValue, SessionConfigSelectOption,
    SessionConfigSelectOptions,
};

pub fn select_options(
    options: &SessionConfigSelectOptions,
) -> impl Iterator<Item = &SessionConfigSelectOption> {
    let (options, groups) = match options {
        SessionConfigSelectOptions::Ungrouped(options) => (options.as_slice(), [].as_slice()),
        SessionConfigSelectOptions::Grouped(groups) => ([].as_slice(), groups.as_slice()),
        _ => ([].as_slice(), [].as_slice()),
    };
    options.iter().chain(groups.iter().flat_map(|group| &group.options))
}

pub fn value_name(option: &SessionConfigOption) -> Cow<'_, str> {
    match &option.kind {
        SessionConfigKind::Select(select) => Cow::Borrowed(
            select_options(&select.options)
                .find(|choice| choice.value == select.current_value)
                .map(|choice| choice.name.as_str())
                .unwrap_or(&select.current_value.0),
        ),
        SessionConfigKind::Boolean(value) => Cow::Owned(format!(
            "{}：{}",
            option.name,
            if value.current_value { "开启" } else { "关闭" }
        )),
        _ => Cow::Borrowed(&option.name),
    }
}

pub fn can_select(option: &SessionConfigOption, value: &SessionConfigOptionValue) -> bool {
    match (&option.kind, value) {
        (SessionConfigKind::Select(select), SessionConfigOptionValue::ValueId { value }) => {
            value != &select.current_value
                && select_options(&select.options).any(|choice| &choice.value == value)
        }
        (SessionConfigKind::Boolean(current), SessionConfigOptionValue::Boolean { value }) => {
            *value != current.current_value
        }
        _ => false,
    }
}
