use scada_core::service::{print_health, ServiceRole};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodedError {
    code: String,
    path: String,
    detail: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::BuilderApi);
        return;
    }

    if let Some(raw) = arg_value(&args, "--map-error") {
        println!("{}", map_condition_error_message(&raw));
        return;
    }

    println!("builder-api skeleton");
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn parse_coded_error(raw: &str) -> Option<CodedError> {
    let code_start = raw.find("code=")? + "code=".len();
    let path_marker = " path=";
    let path_start_marker = raw[code_start..].find(path_marker)? + code_start;
    let code = raw[code_start..path_start_marker].trim().to_string();

    let path_start = path_start_marker + path_marker.len();
    let detail_marker = " detail=";
    let detail_start_marker = raw[path_start..].find(detail_marker)? + path_start;
    let path = raw[path_start..detail_start_marker].trim().to_string();

    let detail_start = detail_start_marker + detail_marker.len();
    let detail = raw[detail_start..].trim().to_string();

    Some(CodedError { code, path, detail })
}

fn map_condition_error_message(raw: &str) -> String {
    let Some(parsed) = parse_coded_error(raw) else {
        return raw.to_string();
    };

    match parsed.code.as_str() {
        "MODIFY_RULE_CONDITION_MISSING_SELECTOR" => {
            format!(
                "invalid modify rule condition at {}: choose op, all, or any",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_VALUE_REQUIRED" => {
            format!(
                "invalid modify rule condition at {}: missing value",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX" => {
            format!(
                "invalid modify rule condition at {}: between requires min and max",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_BETWEEN_RANGE_INVALID" => {
            format!(
                "invalid modify rule condition at {}: min must be less than or equal to max",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_IN_REQUIRES_VALUES" => {
            format!(
                "invalid modify rule condition at {}: in requires non-empty values",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_UNSUPPORTED_OP" => {
            format!(
                "invalid modify rule condition at {}: unsupported operator",
                parsed.path
            )
        }
        _ => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_coded_error_reads_code_path_and_detail() {
        let parsed = parse_coded_error(
            "screen definition error: code=MODIFY_RULE_CONDITION_VALUE_REQUIRED path=object=pump-001 property=color detail=op 'gte' requires value",
        )
        .expect("parsed");

        assert_eq!("MODIFY_RULE_CONDITION_VALUE_REQUIRED", parsed.code);
        assert_eq!("object=pump-001 property=color", parsed.path);
        assert_eq!("op 'gte' requires value", parsed.detail);
    }

    #[test]
    fn map_condition_error_message_returns_user_friendly_text() {
        let mapped = map_condition_error_message(
            "code=MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX path=object=pump-001 property=color detail=op 'between' requires min and max",
        );

        assert_eq!(
            "invalid modify rule condition at object=pump-001 property=color: between requires min and max",
            mapped
        );
    }

    #[test]
    fn map_condition_error_message_keeps_unknown_code_raw() {
        let raw = "code=UNKNOWN_CODE path=object=x property=y detail=unknown";
        assert_eq!(raw, map_condition_error_message(raw));
    }
}
