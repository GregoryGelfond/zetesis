row(1..3). col(1..3).
{p(R,C): col(C)} :- row(R).
:- p(R,C), p(S,D), R < S, R+C = S+D.
