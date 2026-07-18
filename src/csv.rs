//! Minimal RFC4180 CSV encode/decode — handles quoted fields containing
//! commas, quotes, and newlines, which a naive `split(',')` would corrupt.

pub fn parse(input: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.is_empty()));
    rows
}

pub fn field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_quoted_field() {
        let raw = "has, comma\nand \"quotes\"\nand\nnewline";
        let encoded = format!("name,notes\nFlo,{}\n", field(raw));
        let rows = parse(&encoded);
        assert_eq!(rows, vec![
            vec!["name".to_string(), "notes".to_string()],
            vec!["Flo".to_string(), raw.to_string()],
        ]);
    }

    #[test]
    fn parses_plain_rows() {
        let rows = parse("a,b\nc,d\n");
        assert_eq!(rows, vec![vec!["a", "b"], vec!["c", "d"]]);
    }
}
