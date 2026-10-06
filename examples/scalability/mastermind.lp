% Mastermind code breaking: every secret code consistent with the scored guesses.
%
% A secret assigns one of `colors` colours to each of `pegs` positions; colours
% may repeat. Each guess is scored by its black pegs (right colour, right
% position) and white pegs (right colour, wrong position). The answer sets are
% exactly the secrets that would have produced every recorded score.

#const pegs = 6.
#const colors = 6.

peg(1..pegs).
color(1..colors).

% One scored guess: 1 2 3 3 4 5, with no black pegs and three white pegs.
guess(1, 1, 1). guess(1, 2, 2). guess(1, 3, 3).
guess(1, 4, 3). guess(1, 5, 4). guess(1, 6, 5).
score(1, 0, 3).

% Each position of the secret holds exactly one colour.
1 { secret(P, C) : color(C) } 1 :- peg(P).

% Black pegs: positions where guess and secret agree.
black(G, B) :- score(G, _, _), B = #count { P : guess(G, P, C), secret(P, C) }.

% Colour matches: for each colour, the smaller of its counts in the guess and
% in the secret; black and white pegs together account for all of them.
in_guess(G, C, N) :- guess(G, _, C), N = #count { P : guess(G, P, C) }.
in_secret(C, N) :- color(C), N = #count { P : secret(P, C) }.
common(G, C, N) :- in_guess(G, C, N), in_secret(C, M), N <= M.
common(G, C, M) :- in_guess(G, C, N), in_secret(C, M), M < N.
matched(G, T) :- score(G, _, _), T = #sum { N, C : common(G, C, N) }.

:- score(G, B, _), not black(G, B).
:- score(G, B, W), not matched(G, B + W).

#show secret/2.
