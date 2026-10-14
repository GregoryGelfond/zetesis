d(1..2).
{p(X):d(X)}.
p(3) :- not absent.
p(4) | alternative.
out(X) :- p(X), not missing(X).
