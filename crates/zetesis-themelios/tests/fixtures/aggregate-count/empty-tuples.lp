% The empty tuple has two alternative witnesses, while an empty set has none.
{p}.
q(N) :- N = #count { : p; : not p }.
z(N) :- N = #count {}.
