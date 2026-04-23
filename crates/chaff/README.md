# Chaff

For usage, see the documentation and tests.

Chaff is split into multiple parts, all stemming from the `Framework` in `framework.rs`:

- `Framework` (`framework.rs`) - the entrypoint into chaff.
- `Machine` (`machine.rs`) - the specification for a queue machine. Can be thought of as the defence itself.
- `MachineRuntime` (`machine.rs`) - made by the framework, contains runtime information needed by the framework to run the defence.
- `State` (`state.rs`) - a single state in a Chaff `Machine`. Contains an `Action` which is triggered on transition to the state.
- `Action` (`action.rs`) - an `enum` that represents either an action for the `Framework` to take, or for an integrator to take via:
  - `FrameworkAction` (`action.rs`) - an `enum` with all the different actions the `Framework` can take.
  - `IntegratorAction` (`action.rs`) - an `enum` with all the different actions an integrator must implement.
- `TransitionProbs` and `Transition` (`state.rs`) - representes the probabilities of transitioning to different `State`s based on a triggered `Event`.
- `Event` (`event.rs`) - something that happens, i.e. `SendNormal` meaning a real packet has been sent (queued). These can also be deferred by the `Framework`.
- `TimedQueue` (`queue.rs`) - used in the `MachineRuntime`. Allows scheduling of `Action`s for specific times.

Chaff has more going on under the hood. For more information, read the code and documentation.
