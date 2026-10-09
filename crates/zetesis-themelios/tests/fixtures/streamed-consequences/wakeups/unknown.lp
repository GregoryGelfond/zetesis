{h(in_state(v,open),0);h(other(v,closed),0);q}.
valve(v).
:-valve(V),not h(in_state(V,open),0),q.
