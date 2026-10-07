% Mastermind, 6 pegs and 8 colours, seven scored guesses: the colour-match
% count is a nested aggregate (the smaller of two counts, summed per guess).
#const pegs = 6.
#const colors = 8.
peg(1..pegs).
color(1..colors).
code(1, (1,2,3,3,4,5), (0,3)).
code(2, (2,8,6,7,5,4), (1,2)).
code(3, (8,1,2,7,1,3), (1,1)).
code(4, (6,6,6,5,2,3), (1,1)).
code(5, (4,4,6,2,1,2), (0,1)).
code(6, (5,5,4,8,5,3), (1,4)).
code(7, (8,5,8,5,3,4), (6,0)).
guess(G, 1, C) :- code(G, (C,_,_,_,_,_), _).
guess(G, 2, C) :- code(G, (_,C,_,_,_,_), _).
guess(G, 3, C) :- code(G, (_,_,C,_,_,_), _).
guess(G, 4, C) :- code(G, (_,_,_,C,_,_), _).
guess(G, 5, C) :- code(G, (_,_,_,_,C,_), _).
guess(G, 6, C) :- code(G, (_,_,_,_,_,C), _).
1 { secret(P, C) : color(C) } 1 :- peg(P).
black(G, P) :- guess(G, P, C), secret(P, C).
shared(G, C, M) :- guess(G, _, C),
    InGuess = #count { P : guess(G, P, C) },
    InSecret = #count { P : secret(P, C) },
    M = #min { InGuess; InSecret }.
score(G, B, H - B) :- guess(G, _, _),
    B = #count { P : black(G, P) },
    H = #sum { M, C : shared(G, C, M) }.
:- code(G, _, (B, W)), not score(G, B, W).
#show secret/2.
