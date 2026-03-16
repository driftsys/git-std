#[test]
fn trycmd_commit() {
    trycmd::TestCases::new().case("tests/cmd/commit/*.toml");
}
