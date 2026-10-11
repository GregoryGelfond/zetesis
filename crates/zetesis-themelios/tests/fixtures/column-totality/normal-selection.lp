{p(4,2);p(9,3);p(5,2)}.
kept(X,Y):-p(X,Y),X/Y=2.
:-kept(X,Y),X\Y=0.
