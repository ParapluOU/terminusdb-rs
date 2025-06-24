use crate::*;

// use reqwest::*;
// use tokio::*;
//
// #[tokio::test]
// async fn test_code_generation() {
//     let woql_schema_text = reqwest::get("https://raw.githubusercontent.com/terminusdb/terminusdb/main/src/terminus-schema/woql.json").unwrap().text().unwrap();
//     let woql_schema_json: serde_json::Value = serde_json::from_str(woql_schema_text).unwrap();
//     println!("{:#?}", woql_schema_json);
// }

/*
   select([Song], (\
        t(Song, score, Score)\
       ,t(Score, parts, Parts)\
       ,t(Parts, _PartIdx, Part)\
       ,t(Part, beats, Beats)\
       ,t(Beats, _BeatIdx, Beat)\
       ,t(Beat, duration, BeatDuration)\
       ,t(BeatDuration, dots, 0^^xsd:unsignedInt)))
*/
fn create_query() -> Query {
    let var_song = var!(Song);
    let var_score = var!(Score);
    let var_parts = var!(Parts);
    let var_part = var!(Part);
    let var_beats = var!(Beats);
    let var_beat = var!(Beat);
    let var_beat_duration = var!(BeatDuration);

    distinct!(
        [var_song],
        select!(
            [var_song],
            and!(
                t!(var_song, pred!(score), var_score),
                t!(var_score, pred!(parts), var_parts),
                t!(var_parts, var!(_PartIdx), var_part),
                t!(var_part, pred!(beats), var_beats),
                t!(var_beats, var!(_BeatIdx), var_beat),
                t!(var_beat, pred!(duration), var_beat_duration),
                t!(
                    var_beat_duration,
                    pred!(dots),
                    crate::xsd!(0 => UnsignedInt)
                )
            )
        )
    )
    .into()
}

#[test]
fn generate_song_query() {
    let q = create_query();
    let ast = q.to_ast();
    assert_eq!(ast, "distinct([Song],select([Song],(t(Song,'score',Score),t(Score,'parts',Parts),t(Parts,_PartIdx,Part),t(Part,'beats',Beats),t(Beats,_BeatIdx,Beat),t(Beat,'duration',BeatDuration),t(BeatDuration,'dots',0^^xsd:unsignedInt))))".to_string());
}

// #[test]
// fn query_song_count() {
//     let client = TerminusDBClient::connect_catalog(None);
//     let res = client.query("admin/scores", q).unwrap();
//     dbg!("{}", res);
// }
