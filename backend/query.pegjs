query
  = terms:(_ ident)* _ { return terms.map(x => x[1]); }

ident
  = [a-zA-Z][a-zA-Z0-9_]* { return text(); }

_ "whitespace"
  = [ \t\n\r]*
