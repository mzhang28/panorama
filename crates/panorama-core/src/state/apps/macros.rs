macro_rules! abi_funcs {
  ($macro_name:ident) => {
    // TODO: Why is this "env"? How do i use another name
    $macro_name! {
      "env"::get_current_time,
      "env"::print,
      "env"::register_endpoint,
    }
  };
}
