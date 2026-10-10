{p(1..4)}.
valid :- #count { X : p(X) } = 2.
:- not valid.
