use azurite_common::{parse_args, EnvironmentParameter};

#[test]
fn parse_arguments_invalid_usage() {
    let values: Vec<String> = vec!["executable path".to_string()];

    let value = parse_args(values.into_iter());
    assert!(value.is_err())
}

#[test]
fn parse_arguments_file_path() {
    let values: Vec<String> = vec![
        "executable path".to_string(),
        "/some/random/file/path".to_string(),
    ];

    let value = parse_args(values.into_iter());

    assert_eq!(value, Ok(("/some/random/file/path".to_string(), vec![],)));
}

#[test]
fn parse_arguments_file_path_with_environment() {
    let values: Vec<String> = vec![
        "executable path".to_string(),
        "/some/random/file/path".to_string(),
        "--hello-there".to_string(),
        "this will be ignored".to_string(),
        "--another-value=3".to_string(),
        "--another-another-value=hello".to_string(),
    ];

    let value = parse_args(values.into_iter());

    assert_eq!(
        value,
        Ok((
            "/some/random/file/path".to_string(),
            vec![
                EnvironmentParameter {
                    identifier: "hello-there".to_string(),
                    value: "1".to_string(),
                },
                EnvironmentParameter {
                    identifier: "another-value".to_string(),
                    value: "3".to_string(),
                },
                EnvironmentParameter {
                    identifier: "another-another-value".to_string(),
                    value: "hello".to_string(),
                },
            ]
        ))
    );
}
