/// Single compilation unit for all cucumber step definitions.
///
/// Cucumber-cpp uses __COUNTER__ to generate unique class names for each step.
/// When compiled as separate translation units, __COUNTER__ resets per file,
/// causing symbol collisions. Including all steps in one file avoids this.

#include "player_steps.cpp"
#include "table_steps.cpp"
#include "hand_steps.cpp"
#include "saga_steps.cpp"
#include "process_manager_steps.cpp"
#include "projector_steps.cpp"
#include "common_steps.cpp"
