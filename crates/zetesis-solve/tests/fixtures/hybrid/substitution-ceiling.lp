d(1..6).
p(X) | q(X) :- d(X).
:- d(X), not p(X).
