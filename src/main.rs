use patternfly_yew::prelude::{Grid, GridItem, Progress, ProgressMeasureLocation};
use yew::prelude::*;

#[function_component]
fn App() -> Html {
    html!(
        <Grid gutter=true>
        <GridItem cols={[8]}>
        <Progress description="Test" value=0.33 location={ProgressMeasureLocation::Inside} />
        {test()}
        </GridItem>
        <GridItem cols={[4]} rows={[2]}>
            {"cols = 4, rows = 2"}
        </GridItem>
        <GridItem cols={[2]} rows={[3]}>
          {"cols = 2, rows = 3"}
        </GridItem>
        <GridItem cols={[2]}>{"cols = 2"}</GridItem>
        <GridItem cols={[4]}>{"cols = 4"}</GridItem>
        <GridItem cols={[2]}>{"cols = 2"}</GridItem>
        <GridItem cols={[2]}>{"cols = 2"}</GridItem>
        <GridItem cols={[2]}>{"cols = 2"}</GridItem>
        <GridItem cols={[4]}>{"cols = 4"}</GridItem>
        <GridItem cols={[2]}>{"cols = 2"}</GridItem>
        <GridItem cols={[4]}>{"cols = 4"}</GridItem>
        <GridItem cols={[4]}>{"cols = 4"}</GridItem>
    </Grid>
    )
}

fn test() -> Html {
    html!(
        <>
        <h1>{"Hey!!"}</h1>
        </>
    )
}

fn main() {
    yew::Renderer::<App>::new().render();
}
