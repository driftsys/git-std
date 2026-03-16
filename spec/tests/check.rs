#[test]
fn trycmd_check() {
    trycmd::TestCases::new().case("tests/cmd/check/*.toml");
}
