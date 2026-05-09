use pbsm_core::modules::metacognition::controller::MetacognitiveController;
use pbsm_core::modules::metacognition::types::{
    AdjustAttentionRequest, AdjustmentTrigger, EvaluateMemoryValueRequest, ForceForgetRequest,
    ForgetReason,
};

#[tokio::test]
/// 预测验证后注意力参数下降
async fn test_prediction_verified_lowers_attention() {
    let controller = MetacognitiveController::new();

    let initial_status = controller.get_attention_status().await;
    let initial_attention = initial_status.attention_parameter;

    let result = controller
        .adjust_attention(AdjustAttentionRequest {
            delta: Some(-0.1),
            target_value: None,
            trigger: AdjustmentTrigger::PredictionVerified,
            override_mode: None,
        })
        .await
        .unwrap();

    assert!(result.new_value < initial_attention);
}

#[tokio::test]
/// 预测偏差触发后注意力跳升至0.8+
async fn test_prediction_falsified_raises_attention() {
    let controller = MetacognitiveController::new();

    let initial_status = controller.get_attention_status().await;
    let initial_attention = initial_status.attention_parameter;
    assert!((initial_attention - 0.5).abs() < f64::EPSILON);

    let result = controller
        .adjust_attention(AdjustAttentionRequest {
            delta: None,
            target_value: None,
            trigger: AdjustmentTrigger::PredictionDeviation,
            override_mode: None,
        })
        .await
        .unwrap();

    assert!(result.new_value >= 0.8);
}

#[tokio::test]
/// 残差关联的信念价值评分更高
async fn test_residual_drives_value_evaluation() {
    let controller = MetacognitiveController::new();

    controller
        .value_evaluator()
        .register_belief("belief-high-residual", 0.5);
    controller
        .value_evaluator()
        .set_belief_residual_association("belief-high-residual", 0.9);
    controller
        .value_evaluator()
        .set_belief_access_count("belief-high-residual", 5);
    controller
        .value_evaluator()
        .set_belief_last_accessed("belief-high-residual", 1);

    controller
        .value_evaluator()
        .register_belief("belief-no-residual", 0.5);
    controller
        .value_evaluator()
        .set_belief_residual_association("belief-no-residual", 0.0);
    controller
        .value_evaluator()
        .set_belief_access_count("belief-no-residual", 5);
    controller
        .value_evaluator()
        .set_belief_last_accessed("belief-no-residual", 1);

    let result = controller
        .evaluate_memory_value(EvaluateMemoryValueRequest {
            node_ids: Some(vec![
                "belief-high-residual".to_string(),
                "belief-no-residual".to_string(),
            ]),
            all_active: None,
            include_factors: Some(true),
        })
        .await
        .unwrap();

    let high_residual_score = result
        .value_scores
        .iter()
        .find(|s| s.node_id == "belief-high-residual")
        .unwrap()
        .total_score;
    let no_residual_score = result
        .value_scores
        .iter()
        .find(|s| s.node_id == "belief-no-residual")
        .unwrap()
        .total_score;

    assert!(high_residual_score > no_residual_score);
}

#[tokio::test]
/// 快速上下切换后能检测到异常
async fn test_attention_anomaly_detection() {
    let controller = MetacognitiveController::new();

    for i in 0..20 {
        let trigger = if i % 2 == 0 {
            AdjustmentTrigger::PredictionDeviation
        } else {
            AdjustmentTrigger::PredictionVerified
        };
        controller
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: None,
                trigger,
                override_mode: None,
            })
            .await
            .unwrap();
    }

    let report = controller.detect_anomalies(None);
    assert!(report.has_anomalies);
}

#[tokio::test]
/// 有残差关联的信念被延迟遗忘（deferred），无残差的被正常遗忘
async fn test_forgetting_preserves_residual_beliefs() {
    let controller = MetacognitiveController::new();

    controller
        .forgetting_executor()
        .set_belief_age("belief-with-residual", 20);
    controller
        .forgetting_executor()
        .set_belief_residual_association("belief-with-residual", 0.9);

    controller
        .forgetting_executor()
        .set_belief_age("belief-without-residual", 20);

    let result = controller
        .force_forget(ForceForgetRequest {
            node_ids: vec![
                "belief-with-residual".to_string(),
                "belief-without-residual".to_string(),
            ],
            force_flag: None,
            reason: ForgetReason::LowValue,
        })
        .unwrap();

    assert!(result
        .deferred_ids
        .contains(&"belief-with-residual".to_string()));
    assert!(result
        .forgotten_ids
        .contains(&"belief-without-residual".to_string()));
    assert!(!result
        .forgotten_ids
        .contains(&"belief-with-residual".to_string()));
}
