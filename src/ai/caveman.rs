use regex::Regex;
use std::sync::LazyLock;

static PREAMBLE_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)^(com certeza|certamente|claro|sem problemas|perfeito)[!.,\s]+(aqui está|vamos|vou|abaixo)[^\n]*\n+").unwrap(),
        Regex::new(r"(?i)^(sure|certainly|of course|absolutely)[!.,\s]+(here is|i have|let me|below)[^\n]*\n+").unwrap(),
        Regex::new(r"(?i)^(desculpe|peço desculpas|perdão|sinto muito)[!.,\s]+(pelo equívoco|pela confusão|pelo erro)[^\n]*\n+").unwrap(),
        Regex::new(r"(?i)^(i apologize|sorry for the confusion|my apologies)[!.,\s]+[^\n]*\n+").unwrap(),
        Regex::new(r"(?i)^here is the (code|solution|function|updated implementation)[^:\n]*:\s*\n+").unwrap(),
        Regex::new(r"(?i)^aqui está o (código|arquivo|script|exemplo)[^:\n]*:\s*\n+").unwrap(),
    ]
});

static EPILOGUE_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\n+(espero que isso ajude|se precisar de algo mais|qualquer dúvida estou à disposição|avise se precisar)[^\n]*$").unwrap(),
        Regex::new(r"(?i)\n+(hope this helps|let me know if you have any questions|feel free to ask)[^\n]*$").unwrap(),
    ]
});

/// StripProlixity removes polite filler, preambles, and conversational fluff from the response.
pub fn strip_prolixity(response: &str) -> String {
    let mut trimmed = response.trim().to_string();

    // Strip preambles
    for re in PREAMBLE_REGEXES.iter() {
        trimmed = re.replace(&trimmed, "").trim().to_string();
    }

    // Strip epilogues
    for re in EPILOGUE_REGEXES.iter() {
        trimmed = re.replace(&trimmed, "").trim().to_string();
    }

    trimmed
}

/// SystemDirective returns a concise prompt constraint to enforce direct answers from the model.
pub fn system_directive() -> &'static str {
    "Be extremely concise. No conversational filler, preambles ('Sure thing', 'Here is'), apologies, or pleasantries. Output code, file paths, commands, and direct answers only."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_prolixity() {
        let input = "Com certeza! Aqui está o código:\nprint('hello')\nEspero que isso ajude!";
        let clean = strip_prolixity(input);
        assert_eq!(clean, "print('hello')");
    }
}
