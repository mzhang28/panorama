query
  = ident

ident
  = [a-zA-Z][a-zA-Z0-9_]* { return text(); }

_ "whitespace"
  = [ \t\n\r]*
