#![allow(dead_code)]

use crate::{
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const CALCULATOR_SCORE: i32 = 980;

pub(crate) fn search_calculator(query: &str, limit: usize) -> Vec<SearchResult> {
    let limit = settings::normalize_result_limit(limit);
    let expression = query.trim();

    if limit == 0 || !looks_like_calculator_query(expression) {
        return Vec::new();
    }

    let Ok(value) = evaluate_expression(expression) else {
        return Vec::new();
    };

    if !value.is_finite() {
        return Vec::new();
    }

    let result = format_number(value);
    let normalized_expression = normalize_expression(expression);

    vec![SearchResult {
        id: format!("calculator:{normalized_expression}"),
        title: result.clone(),
        subtitle: Some(expression.to_owned()),
        icon: Some("calculator".to_owned()),
        source: SearchSource::Calculator,
        action: SearchAction::CopyText,
        path: None,
        score: CALCULATOR_SCORE,
        metadata: Some(SearchMetadata::Calculator {
            expression: expression.to_owned(),
            result: result.clone(),
            copy_text: result,
        }),
    }]
}

fn looks_like_calculator_query(query: &str) -> bool {
    !query.trim().is_empty()
        && query.chars().any(|ch| ch.is_ascii_digit())
        && query
            .chars()
            .any(|ch| matches!(ch, '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')'))
        && !query.chars().any(|ch| ch.is_alphabetic())
}

fn evaluate_expression(expression: &str) -> Result<f64, CalculatorError> {
    let mut parser = Parser::new(expression);
    let value = parser.parse_expression()?;
    parser.skip_whitespace();

    if parser.is_at_end() {
        Ok(value)
    } else {
        Err(CalculatorError)
    }
}

fn format_number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-10 {
        return format!("{value:.0}");
    }

    let formatted = format!("{value:.12}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn normalize_expression(expression: &str) -> String {
    expression
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CalculatorError;

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn new(expression: &str) -> Self {
        Self {
            chars: expression.chars().collect(),
            position: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<f64, CalculatorError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<f64, CalculatorError> {
        let mut value = self.parse_mul_div()?;

        loop {
            if self.consume('+') {
                value += self.parse_mul_div()?;
            } else if self.consume('-') {
                value -= self.parse_mul_div()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_mul_div(&mut self) -> Result<f64, CalculatorError> {
        let mut value = self.parse_power()?;

        loop {
            if self.consume('*') {
                value *= self.parse_power()?;
            } else if self.consume('/') {
                let rhs = self.parse_power()?;

                if rhs == 0.0 {
                    return Err(CalculatorError);
                }

                value /= rhs;
            } else if self.consume('%') {
                let rhs = self.parse_power()?;

                if rhs == 0.0 {
                    return Err(CalculatorError);
                }

                value %= rhs;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_power(&mut self) -> Result<f64, CalculatorError> {
        let base = self.parse_unary()?;

        if self.consume('^') {
            let exponent = self.parse_power()?;
            Ok(base.powf(exponent))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<f64, CalculatorError> {
        if self.consume('+') {
            self.parse_unary()
        } else if self.consume('-') {
            Ok(-self.parse_unary()?)
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<f64, CalculatorError> {
        if self.consume('(') {
            let value = self.parse_expression()?;

            if self.consume(')') {
                return Ok(value);
            }

            return Err(CalculatorError);
        }

        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<f64, CalculatorError> {
        self.skip_whitespace();
        let start = self.position;
        let mut has_digit = false;

        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            has_digit = true;
            self.position += 1;
        }

        if self.peek() == Some('.') {
            self.position += 1;

            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                has_digit = true;
                self.position += 1;
            }
        }

        if !has_digit {
            return Err(CalculatorError);
        }

        self.chars[start..self.position]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .map_err(|_| CalculatorError)
    }

    fn consume(&mut self, expected: char) -> bool {
        self.skip_whitespace();

        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.chars.len()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn calculator_result(query: &str) -> Option<SearchResult> {
        search_calculator(query, 8).into_iter().next()
    }

    #[test]
    fn simple_addition_returns_result() {
        assert_eq!(calculator_result("2+2").expect("result").title, "4");
    }

    #[test]
    fn parentheses_and_precedence_are_supported() {
        assert_eq!(
            calculator_result("2 * (3 + 4)").expect("result").title,
            "14"
        );
        assert_eq!(calculator_result("2 + 3 * 4").expect("result").title, "14");
    }

    #[test]
    fn exponentiation_is_right_associative() {
        assert_eq!(calculator_result("2^8").expect("result").title, "256");
        assert_eq!(calculator_result("2^3^2").expect("result").title, "512");
    }

    #[test]
    fn unary_operators_are_supported() {
        assert_eq!(calculator_result("-5 + 12").expect("result").title, "7");
        assert_eq!(calculator_result("+5 + -2").expect("result").title, "3");
    }

    #[test]
    fn division_and_modulo_work() {
        assert_eq!(calculator_result("10 / 4").expect("result").title, "2.5");
        assert_eq!(calculator_result("10 % 3").expect("result").title, "1");
    }

    #[test]
    fn division_and_modulo_by_zero_return_no_result() {
        assert!(calculator_result("10 / 0").is_none());
        assert!(calculator_result("10 % 0").is_none());
    }

    #[test]
    fn invalid_syntax_returns_no_result() {
        for query in ["2+", "2 * (3 + 4", "2..3 + 1", "2 2 + 1"] {
            assert!(
                calculator_result(query).is_none(),
                "{query} should be rejected"
            );
        }
    }

    #[test]
    fn plain_text_and_identifiers_return_no_result() {
        for query in ["", "firefox", "report 2", "sqrt(4)", "2 + two"] {
            assert!(
                calculator_result(query).is_none(),
                "{query} should not look like calculator input"
            );
        }
    }

    #[test]
    fn non_finite_results_return_no_result() {
        assert!(calculator_result("10^10000").is_none());
    }

    #[test]
    fn decimal_formatting_trims_unnecessary_zeroes() {
        assert_eq!(
            calculator_result("1.50 + 1.25").expect("result").title,
            "2.75"
        );
        assert_eq!(calculator_result("0.1 + 0.2").expect("result").title, "0.3");
    }

    #[test]
    fn zero_limit_uses_default_limit() {
        assert_eq!(calculator_result("2+2").expect("result").title, "4");
        assert_eq!(
            search_calculator("2+2", 0).first().expect("result").title,
            "4"
        );
    }

    #[test]
    fn result_uses_expected_frontend_shape() {
        let result = calculator_result(" 2 + 2 ").expect("result");

        assert_eq!(
            serde_json::to_value(result).expect("result should serialize"),
            json!({
                "id": "calculator:2+2",
                "title": "4",
                "subtitle": "2 + 2",
                "icon": "calculator",
                "source": "calculator",
                "action": "copy_text",
                "path": null,
                "score": 980,
                "metadata": {
                    "kind": "calculator",
                    "expression": "2 + 2",
                    "result": "4",
                    "copy_text": "4"
                }
            })
        );
    }
}
