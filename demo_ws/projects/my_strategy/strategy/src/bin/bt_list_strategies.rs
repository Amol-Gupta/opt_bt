use my_strategy as _;

fn main() {
    let metadata = bt_strategy_sdk::all_metadata();
    let json = serde_json::to_string_pretty(&metadata).expect("serialize strategy metadata");
    println!("{}", json);
}
