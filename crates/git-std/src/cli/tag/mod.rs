mod configuration;
mod identity;
mod manifests;
mod output;
mod plan;
mod remote;
mod snapshot;
mod tags;
mod types;
mod versions;

pub use types::TagOptions;

/// Tag an existing release commit. Returns the process exit code.
pub fn run(options: &TagOptions) -> i32 {
    let mut plan = types::TagPlan::default();
    let result = (|| -> anyhow::Result<()> {
        let root = plan::repository()?;
        plan::build(&root, options, &mut plan)?;
        tags::preflight(&root, &mut plan, options.sign)?;
        if options.push.is_some() {
            remote::preflight(&root, &mut plan, options.sign)?;
        }
        if options.dry_run {
            return Ok(());
        }
        tags::apply(&root, &mut plan, options.sign)?;
        if options.push.is_some() {
            remote::publish(&root, &mut plan)?;
        }
        Ok(())
    })();
    let exit = match &result {
        Ok(()) => 0,
        Err(error)
            if error.downcast_ref::<types::Refusal>().is_some()
                && !plan
                    .tags
                    .iter()
                    .any(|t| matches!(t.local, "created" | "adopted")) =>
        {
            1
        }
        Err(_) => 2,
    };
    let error = result.err().map(|e| format!("{e:#}"));
    output::print(options, &plan, exit, error.as_deref());
    exit
}
