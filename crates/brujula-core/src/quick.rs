//! Quick answers in the Spotlight, like Arc's command bar: a search inside a site by typing its
//! short name first (`yt gatos`, `gh brujula`) and a calculator (`2+2*3`, `(10-4)/3`).

use url::Url;

/// A site that can be searched from the Spotlight: what to type first, its name, and the search
/// address with `%s` for the query.
#[derive(Debug, Clone, Copy)]
pub struct SiteSearch {
    pub keyword: &'static str,
    pub name: &'static str,
    pub template: &'static str,
}

/// One site per line: `keyword|name|address`. A table of data, not of repeated literals.
const TABLE: &str = "\
yt|YouTube|https://www.youtube.com/results?search_query=%s
gh|GitHub|https://github.com/search?q=%s
w|Wikipedia|https://es.wikipedia.org/w/index.php?search=%s
mdn|MDN|https://developer.mozilla.org/es/search?q=%s
so|Stack Overflow|https://stackoverflow.com/search?q=%s
hn|Hacker News|https://hn.algolia.com/?q=%s
arch|Arch Wiki|https://wiki.archlinux.org/index.php?search=%s
aur|AUR|https://aur.archlinux.org/packages?K=%s
crates|crates.io|https://crates.io/search?q=%s
npm|npm|https://www.npmjs.com/search?q=%s
mapa|OpenStreetMap|https://www.openstreetmap.org/search?query=%s
rae|Diccionario de la RAE|https://dle.rae.es/srv/search?w=%s
amazon|Amazon|https://www.amazon.es/s?k=%s";

/// The sites that can be searched by their short name.
pub fn sites() -> impl Iterator<Item = SiteSearch> {
    TABLE.lines().filter_map(|line| {
        let mut parts = line.splitn(3, '|');
        Some(SiteSearch {
            keyword: parts.next()?,
            name: parts.next()?,
            template: parts.next()?,
        })
    })
}

/// A search inside a site: `yt gatos` becomes the results page of YouTube for «gatos».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteHit {
    pub name: &'static str,
    pub query: String,
    pub url: String,
}

/// `text` when it starts with a site's short name, a space and something to look for.
pub fn site_search(text: &str) -> Option<SiteHit> {
    let text = text.trim_start();
    let (word, rest) = text.split_once(char::is_whitespace)?;
    let query = rest.trim();
    if query.is_empty() || query.chars().count() > 200 {
        return None;
    }
    let word = word.to_lowercase();
    let site = sites().find(|s| s.keyword == word)?;
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let url = Url::parse(&site.template.replace("%s", &encoded)).ok()?;
    Some(SiteHit {
        name: site.name,
        query: query.to_owned(),
        url: url.to_string(),
    })
}

/// The answer to an arithmetic expression the user typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calc {
    pub expression: String,
    pub result: String,
}

const MAX_EXPRESSION: usize = 120;
const MAX_DEPTH: usize = 24;

/// Evaluates `text` when it is arithmetic with an operation in it (`+ - * / % ^`, brackets, `×`,
/// `÷`), not just a number. Anything else (a word, an address, a version like `1.2.3`) is `None`.
pub fn calculate(text: &str) -> Option<Calc> {
    let expression = text.trim();
    if expression.is_empty() || expression.chars().count() > MAX_EXPRESSION {
        return None;
    }
    let mut p = Parser {
        chars: expression
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| match c {
                '×' | 'x' | 'X' => '*',
                '÷' => '/',
                ',' => '.',
                c => c,
            })
            .collect(),
        at: 0,
        operations: 0,
    };
    // A bare `x` is only a multiplication between two numbers: `2x3`, never a word.
    if expression
        .chars()
        .any(|c| c.is_alphabetic() && !matches!(c, 'x' | 'X'))
    {
        return None;
    }
    if !p.chars.iter().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let value = p.expression(0)?;
    if p.at != p.chars.len() || p.operations == 0 || !value.is_finite() {
        return None;
    }
    Some(Calc {
        expression: expression.to_owned(),
        result: format_number(value),
    })
}

/// 7 stays 7 and 0.1+0.2 shows 0.3: rounded to 10 decimals, with no trailing zeros.
fn format_number(v: f64) -> String {
    if v.abs() >= 1e15 {
        return format!("{v:e}");
    }
    let s = format!("{v:.10}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    /// How many operators were seen: a lone number, even in brackets, is not a calculation.
    operations: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    /// `term (('+' | '-') term)*`
    fn expression(&mut self, depth: usize) -> Option<f64> {
        if depth > MAX_DEPTH {
            return None;
        }
        let mut value = self.term(depth)?;
        while let Some(op @ ('+' | '-')) = self.peek() {
            self.at += 1;
            self.operations += 1;
            let rhs = self.term(depth)?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    /// `power (('*' | '/' | '%') power)*`
    fn term(&mut self, depth: usize) -> Option<f64> {
        let mut value = self.power(depth)?;
        while let Some(op @ ('*' | '/' | '%')) = self.peek() {
            self.at += 1;
            self.operations += 1;
            let rhs = self.power(depth)?;
            value = match op {
                '*' => value * rhs,
                '/' if rhs == 0.0 => return None,
                '/' => value / rhs,
                _ if rhs == 0.0 => return None,
                _ => value % rhs,
            };
        }
        Some(value)
    }

    /// `unary ('^' power)?`, right to left.
    fn power(&mut self, depth: usize) -> Option<f64> {
        let base = self.unary(depth)?;
        if self.peek() == Some('^') {
            self.at += 1;
            self.operations += 1;
            let exp = self.power(depth + 1)?;
            return Some(base.powf(exp));
        }
        Some(base)
    }

    fn unary(&mut self, depth: usize) -> Option<f64> {
        match self.peek()? {
            '-' => {
                self.at += 1;
                Some(-self.unary(depth + 1)?)
            }
            '+' => {
                self.at += 1;
                self.unary(depth + 1)
            }
            '(' => {
                self.at += 1;
                let v = self.expression(depth + 1)?;
                if self.peek() != Some(')') {
                    return None;
                }
                self.at += 1;
                Some(v)
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.at;
        let mut dots = 0;
        while let Some(c) = self.peek() {
            if c == '.' {
                dots += 1;
            } else if !c.is_ascii_digit() {
                break;
            }
            self.at += 1;
        }
        if dots > 1 || self.at == start {
            return None;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(text: &str) -> Option<String> {
        calculate(text).map(|c| c.result)
    }

    #[test]
    fn a_keyword_a_space_and_a_query_search_inside_the_site() {
        let hit = site_search("yt gatos graciosos").unwrap();
        assert_eq!(hit.name, "YouTube");
        assert_eq!(
            hit.url,
            "https://www.youtube.com/results?search_query=gatos+graciosos"
        );
        assert_eq!(site_search("GH brujula").unwrap().name, "GitHub");
        assert_eq!(
            site_search("  so rust&c=1").unwrap().url,
            "https://stackoverflow.com/search?q=rust%26c%3D1"
        );
    }

    #[test]
    fn without_a_query_or_with_another_word_it_is_a_normal_search() {
        for text in ["yt", "yt ", "ytx gatos", "youtube gatos", "", "hola yt"] {
            assert_eq!(site_search(text), None, "{text}");
        }
    }

    #[test]
    fn the_table_is_https_with_a_query_and_no_repeated_keyword() {
        let mut seen = std::collections::HashSet::new();
        for s in sites() {
            assert!(s.template.starts_with("https://"), "{}", s.keyword);
            assert!(s.template.matches("%s").count() == 1, "{}", s.keyword);
            assert!(seen.insert(s.keyword), "{}", s.keyword);
            assert!(site_search(&format!("{} prueba", s.keyword)).is_some());
        }
    }

    #[test]
    fn arithmetic_is_calculated() {
        assert_eq!(calc("2+2").as_deref(), Some("4"));
        assert_eq!(calc("2 + 3 * 4").as_deref(), Some("14"));
        assert_eq!(calc("(2+3)*4").as_deref(), Some("20"));
        assert_eq!(calc("10/4").as_deref(), Some("2.5"));
        assert_eq!(calc("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(calc("2^3^2").as_deref(), Some("512"));
        assert_eq!(calc("-3+5").as_deref(), Some("2"));
        assert_eq!(calc("7%3").as_deref(), Some("1"));
        assert_eq!(calc("1,5*2").as_deref(), Some("3"));
        assert_eq!(calc("6×7").as_deref(), Some("42"));
        assert_eq!(calc("2x3").as_deref(), Some("6"));
        assert_eq!(calc("8÷2").as_deref(), Some("4"));
    }

    #[test]
    fn things_that_are_not_calculations_are_left_alone() {
        for text in [
            "2024",
            "-5",
            "(5)",
            "1.2.3",
            "10.0.0.1",
            "example.com",
            "hola+mundo",
            "2+",
            "(2+3",
            "2+3)",
            "1/0",
            "5%0",
            "x",
            "",
            "   ",
            "++",
        ] {
            assert_eq!(calc(text), None, "{text}");
        }
    }

    #[test]
    fn huge_or_deep_input_is_refused() {
        assert_eq!(calc(&"(".repeat(50)), None);
        let deep = format!("{}1+1{}", "(".repeat(40), ")".repeat(40));
        assert_eq!(calc(&deep), None);
        assert_eq!(calc(&"1+".repeat(200)), None);
        assert_eq!(calc("9^9^9^9"), None, "not finite");
    }

    #[test]
    fn the_answer_keeps_what_was_typed() {
        let c = calculate(" 2 + 2 ").unwrap();
        assert_eq!((c.expression.as_str(), c.result.as_str()), ("2 + 2", "4"));
    }
}
