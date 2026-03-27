#[test]
fn trycmd_config() {
    trycmd::TestCases::new().case("tests/cmd/config/*.toml");
}
