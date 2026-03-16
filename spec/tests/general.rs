#[test]
fn trycmd_general() {
    trycmd::TestCases::new().case("tests/cmd/general/*.toml");
}
