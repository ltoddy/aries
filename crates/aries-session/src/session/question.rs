pub fn resume_input(question: &str, answer: &str) -> String {
    format!(
        "The user answered the question you asked earlier:\n\nQuestion: {question}\nAnswer: {answer}"
    )
}
