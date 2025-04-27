#[async_recursion::async_recursion]
async fn enter_state(
    &self,
    state_id: &S,
    event: Option<&E>,
    context: Arc<RwLock<C>>,
) -> Result<()> {
    // ... (existing checks) ...

    self.execute_entry_actions(state_id, event, context.clone()).await?;

    if let Some(state) = self.states.get(state_id) {
        if state.state_type == RuStateType::Compound {
            if let Some(initial_child_id) = &state.initial {
                let initial_child_s = S::from(initial_child_id.clone()); // Keep for now, may error
                // Pass Some(event) for recursive calls
                self.enter_state(&initial_child_s, event, context.clone()).await?;
            } else if let Some(history_child_id) = self.history.get(state_id.to_string().as_str()) {
                 self.enter_state(history_child_id, event, context.clone()).await?;
             }
        } else if state.state_type == RuStateType::Parallel {
             for child_id_str in &state.children {
                 let child_id_s = S::from(child_id_str.clone()); // Keep for now, may error
                 self.enter_state(&child_id_s, event, context.clone()).await?;
             }
         }
    }
    Ok(())
}

async fn execute_entry_actions(
    &self,
    state_id: &S,
    event: Option<&E>,
    context: Arc<RwLock<C>>,
) -> Result<(), Error> {
    if let Some(actions) = self.entry_actions.get(state_id.to_string().as_str()) {
        if let Some(actual_event) = event {
            // Only execute if event is Some
            for action in actions {
                action.execute(context.clone(), actual_event).await;
            }
        } else {
            // Decide how to handle actions during initialization (event is None)
            // Option: Skip all actions?
            // Option: Modify Action::execute to handle Option<&E>?
            // For now, skipping if event is None.
        }
    }
    Ok(())
} 