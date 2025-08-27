macro_rules! pred_matches {
    ($p:pat => $e:expr) => {
        |x| if let $p = x { Some($e) } else { None }
    };
}
