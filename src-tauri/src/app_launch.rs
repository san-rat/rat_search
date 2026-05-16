use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::app_discovery::{AppCatalog, AppRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LaunchResult {
    pub(crate) app_id: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedLaunch {
    program: String,
    args: Vec<String>,
}

pub(crate) fn launch_app(catalog: &AppCatalog, app_id: &str) -> Result<LaunchResult, String> {
    let app = catalog
        .find_by_id(app_id)
        .ok_or_else(|| format!("app '{app_id}' was not found"))?;
    let prepared = prepare_launch(app)?;

    Command::new(&prepared.program)
        .args(&prepared.args)
        .spawn()
        .map_err(|error| format!("failed to launch '{}': {error}", app.name))?;

    Ok(LaunchResult {
        app_id: app.id.clone(),
        name: app.name.clone(),
    })
}

fn prepare_launch(app: &AppRecord) -> Result<PreparedLaunch, String> {
    if app.terminal {
        return Err(format!(
            "'{}' requires a terminal, which is not supported in Rat Search v0.1",
            app.name
        ));
    }

    prepare_exec(&app.exec)
}

fn prepare_exec(exec: &str) -> Result<PreparedLaunch, String> {
    let args = split_exec_args(exec)?
        .into_iter()
        .map(|arg| strip_field_codes(&arg))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();

    let Some((program, args)) = args.split_first() else {
        return Err("desktop entry Exec command is empty".to_owned());
    };

    Ok(PreparedLaunch {
        program: program.to_owned(),
        args: args.to_vec(),
    })
}

fn split_exec_args(exec: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut in_token = false;
    let mut chars = exec.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let Some(escaped) = chars.next() else {
                    return Err("desktop entry Exec command ends with an escape".to_owned());
                };

                current.push(escaped);
                in_token = true;
            }
            '\'' | '"' if quote == Some(ch) => {
                quote = None;
                in_token = true;
            }
            '\'' | '"' if quote.is_none() => {
                quote = Some(ch);
                in_token = true;
            }
            ch if ch.is_whitespace() && quote.is_none() => {
                if in_token {
                    args.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            _ => {
                current.push(ch);
                in_token = true;
            }
        }
    }

    if let Some(quote) = quote {
        return Err(format!(
            "desktop entry Exec command has an unclosed {quote} quote"
        ));
    }

    if in_token {
        args.push(current);
    }

    Ok(args)
}

fn strip_field_codes(arg: &str) -> Result<String, String> {
    let mut stripped = String::new();
    let mut chars = arg.chars();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            stripped.push(ch);
            continue;
        }

        let Some(code) = chars.next() else {
            return Err("desktop entry Exec command has a trailing '%' field code".to_owned());
        };

        match code {
            '%' => stripped.push('%'),
            'f' | 'F' | 'u' | 'U' | 'i' | 'c' | 'k' => {}
            _ => {
                return Err(format!("unsupported desktop entry field code '%{code}'"));
            }
        }
    }

    Ok(stripped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, exec: &str, terminal: bool) -> AppRecord {
        AppRecord {
            id: id.to_owned(),
            name: "Example App".to_owned(),
            generic_name: None,
            comment: None,
            exec: exec.to_owned(),
            icon: None,
            categories: Vec::new(),
            keywords: Vec::new(),
            desktop_file_path: format!("/tmp/{id}"),
            terminal,
        }
    }

    #[test]
    fn finds_app_by_id() {
        let catalog = AppCatalog {
            apps: vec![app("example.desktop", "example", false)],
        };

        assert_eq!(
            catalog
                .find_by_id("example.desktop")
                .map(|app| app.name.as_str()),
            Some("Example App")
        );
    }

    #[test]
    fn unknown_app_id_returns_an_error() {
        let catalog = AppCatalog::default();

        let error = launch_app(&catalog, "missing.desktop").expect_err("missing app should fail");

        assert!(error.contains("missing.desktop"));
        assert!(error.contains("not found"));
    }

    #[test]
    fn terminal_apps_return_an_error() {
        let error = prepare_launch(&app("terminal.desktop", "terminal-app", true))
            .expect_err("should fail");

        assert!(error.contains("requires a terminal"));
        assert!(error.contains("not supported"));
    }

    #[test]
    fn simple_exec_parses_into_program_and_args() {
        let prepared = prepare_exec("firefox --new-window").expect("exec should parse");

        assert_eq!(prepared.program, "firefox");
        assert_eq!(prepared.args, ["--new-window"]);
    }

    #[test]
    fn quoted_args_parse_correctly() {
        let prepared = prepare_exec(r#"code "--user-data-dir=/tmp/Rat Search" 'project file'"#)
            .expect("exec should parse");

        assert_eq!(prepared.program, "code");
        assert_eq!(
            prepared.args,
            ["--user-data-dir=/tmp/Rat Search", "project file"]
        );
    }

    #[test]
    fn field_codes_are_stripped() {
        let prepared = prepare_exec("app %f --many=%F %u %U --icon %i --name %c --file %k")
            .expect("exec should parse");

        assert_eq!(prepared.program, "app");
        assert_eq!(prepared.args, ["--many=", "--icon", "--name", "--file"]);
    }

    #[test]
    fn escaped_percent_becomes_literal_percent() {
        let prepared = prepare_exec("app %% --value=100%%").expect("exec should parse");

        assert_eq!(prepared.program, "app");
        assert_eq!(prepared.args, ["%", "--value=100%"]);
    }

    #[test]
    fn unclosed_quotes_return_an_error() {
        let error = prepare_exec(r#"app "unterminated"#).expect_err("exec should fail");

        assert!(error.contains("unclosed"));
    }

    #[test]
    fn empty_exec_after_cleanup_returns_an_error() {
        let error = prepare_exec("%U %i %c %k").expect_err("exec should fail");

        assert!(error.contains("empty"));
    }
}
