use slc::{ParsedProject, Project, SDK};

fn main() {
    let sdk_path = "/home/dominic/Repositories/gecko_sdk/gecko_sdk.slcs";
    let sdk = SDK::parse(sdk_path).unwrap();

    let project_path = "/home/dominic/SimplicityStudio/v5_workspace/empty/empty.slcp";
    let project = Project::parse(project_path).unwrap();

    // FIXME: Compute target dir for files

    let resolved = project.resolve_components(&sdk);
    let parsed = ParsedProject::new(&sdk, &project, &resolved);
    parsed.generate_templates("test").expect("failed to generate templates");

    // println!("{:#?}", parsed);
}
