% #script (python)
% import clingo

% def my_min(n1, n2):
%     return clingo.Number(min(n1.number, n2.number))

% #end.

color_choice(1..c).
position_index(1..p).

1 = { goal_color(P, S) : color_choice(S) } :- position_index(P).

%Exclude repeated colors...
% :- goal_color(P1, S), goal_color(P2, S), P1 != P2.

%% ===================

black_position(G, P) :-
    color_guess(G, P, S),
    goal_color(P, S).

color_minimum(G, S, MIN) :-
    color_guess(G, _, S),
    % Guess = { color_guess(G, _, S) },
    % Code = { goal_color(_, S) },
    Guess = #count{ 1, P : color_guess(G, P, S) },
    Code = #count{ 1, P : goal_color(P, S) },
    MIN = #min{ Code; Guess }.

feedback(G, B, W-B) :-
    color_guess(G, _, _),
    B = #count{ 1, P : black_position(G, P) },
    W = #sum{ M, S : color_minimum(G, S, M) }.

% #show goal_color/2.
% #show color_guess/3.
% #show feedback/3.
