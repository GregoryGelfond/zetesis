p:-#count{1:p}>=0.
{a}.
n(N):-N=#count{1:p;2:a}.
#minimize{1@2,anchor:p;N@1,k:n(N)}.
#show.
#show count(X):f(X)=f(Y),n(Y).
