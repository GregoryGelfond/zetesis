% Repeated witnesses coalesce, but equal first fields do not merge tuple tails.
d(a).
d(b).
{p}.
q(N) :- N = #count { 7,X : d(X); 7,X : d(X); 7,a : p }.
