# Noton planning dossier

The owner selected **A: free canvas + editable linear view, with movable ink sketch blocks**.
Start with [the architecture](architecture.md); [the page-model comparison](page-options.md)
records the accepted decision and alternatives.
[Feature choices](feature-choices.md) records tables, ink and all three desktop platforms.
[CI adaptation](ci-plan.md) specifies reuse of Skribisto's workflows.
[Validation and acceptance](validation-plan.md) defines what completion must demonstrate.
[Evidence](evidence.md) separates verified current APIs from proposals and unperformed tests.

The [core Qleany proposal](qleany-core-proposal.yaml) is a schema-6, CLI-validated design
artifact for the proposed local canvas model, not a generation-ready feature implementation.

Implementation now lives in `/home/cyril/Devel/noton` on branch
`codex/noton-architecture`. These documents retain the planned acceptance criteria;
they are not a claim that every criterion has passed. Current dependency setup and
API verification are recorded in [Teksilo integration](teksilo-integration.md).

Current implementation coverage: [user-story and visual audit](ui-audit.md).
