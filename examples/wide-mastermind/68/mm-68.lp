%% 531441, 60480 without duplicate colors
#const p = 6.
#const c = 8.

%% ===================

next_guess(C1, C2, C3, C4, C5, C6) :-
    goal_color(1, C1),
    goal_color(2, C2),
    goal_color(3, C3),
    goal_color(4, C4),
    goal_color(5, C5),
    goal_color(6, C6).

color_guess(G, 1, C1) :- input(G, (C1, _, _, _, _, _), _).
color_guess(G, 2, C2) :- input(G, (_, C2, _, _, _, _), _).
color_guess(G, 3, C3) :- input(G, (_, _, C3, _, _, _), _).
color_guess(G, 4, C4) :- input(G, (_, _, _, C4, _, _), _).
color_guess(G, 5, C5) :- input(G, (_, _, _, _, C5, _), _).
color_guess(G, 6, C6) :- input(G, (_, _, _, _, _, C6), _).

:- input(G, _, (B, W)), not feedback(G, B, W).

#show next_guess/6.

