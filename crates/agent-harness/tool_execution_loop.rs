use std::future::Future;

/// Runs model/tool turns until the model stops requesting tools or the turn
/// budget is exhausted. The callback owns persistence and tool execution.
pub(crate) async fn run_tool_execution_loop<F, Fut>(
    max_turns: usize,
    mut run_turn: F,
) -> Result<usize, ToolLoopError>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<bool, String>>,
{
    for turn in 0..max_turns {
        let requested_tools = run_turn(turn).await.map_err(ToolLoopError::Turn)?;
        if !requested_tools {
            return Ok(turn + 1);
        }
    }

    Err(ToolLoopError::MaxTurnsExceeded { max_turns })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ToolLoopError {
    Turn(String),
    MaxTurnsExceeded { max_turns: usize },
}

impl std::fmt::Display for ToolLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Turn(message) => write!(f, "tool execution turn failed: {message}"),
            Self::MaxTurnsExceeded { max_turns } => {
                write!(f, "tool execution exceeded {max_turns} model turns")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completes_after_a_turn_without_tool_calls() {
        let mut calls = 0;
        let turns = run_tool_execution_loop(4, |_| {
            calls += 1;
            async { Ok(false) }
        })
        .await
        .unwrap();

        assert_eq!(turns, 1);
        assert_eq!(calls, 1);
    }

    #[tokio::test]
    async fn continues_through_multiple_tool_call_turns() {
        let mut outcomes = [true, true, false].into_iter();
        let mut calls = 0;
        let turns = run_tool_execution_loop(4, |_| {
            calls += 1;
            let requested_tools = outcomes.next().unwrap();
            async move { Ok(requested_tools) }
        })
        .await
        .unwrap();

        assert_eq!(turns, 3);
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn propagates_turn_errors_without_running_more_turns() {
        let mut calls = 0;
        let error = run_tool_execution_loop(4, |_| {
            calls += 1;
            async { Err("provider unavailable".to_string()) }
        })
        .await
        .unwrap_err();

        assert_eq!(error, ToolLoopError::Turn("provider unavailable".into()));
        assert_eq!(calls, 1);
    }

    #[tokio::test]
    async fn stops_at_the_configured_turn_limit() {
        let mut calls = 0;
        let error = run_tool_execution_loop(2, |_| {
            calls += 1;
            async { Ok(true) }
        })
        .await
        .unwrap_err();

        assert_eq!(error, ToolLoopError::MaxTurnsExceeded { max_turns: 2 });
        assert_eq!(calls, 2);
    }

    #[tokio::test]
    async fn zero_turn_budget_does_not_invoke_the_callback() {
        let mut calls = 0;
        let error = run_tool_execution_loop(0, |_| {
            calls += 1;
            async { Ok(false) }
        })
        .await
        .unwrap_err();

        assert_eq!(error, ToolLoopError::MaxTurnsExceeded { max_turns: 0 });
        assert_eq!(calls, 0);
    }
}
