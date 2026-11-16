use reqwest::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};
use crate::helpers::spawn_app;

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app().await;

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (body, error_message) in test_cases {
        let response = app.post_subscriptions(body.into()).await;
        assert_eq!(400, response.status().as_u16());
        // assert_eq!(error_message, response.text().await.unwrap());
    }
}

#[tokio::test]
async fn subscriber_returns_a_400_when_fields_are_present_but_invalid() {
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = app.post_subscriptions(body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not respond with 200 OK when the payload was {}.",
            description
        );
    }
}

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // Arrange
    let app = spawn_app().await;
    let email = format!("{}@example.com", uuid::Uuid::new_v4());
    let body = format!("name=le%20guin&email={}", email.replace('@', "%40"));

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act
    let response = app.post_subscriptions(body).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscriber_persists_the_new_subscriber() {
    let app = spawn_app().await;
    let email = format!("{}@example.com", uuid::Uuid::new_v4());
    let body = format!("name=le%20guin&email={}", email.replace('@', "%40"));

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act
    let response = app.post_subscriptions(body).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!(
        "SELECT email, name, status FROM subscriptions WHERE email = $1",
        email
    )
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, email);
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "pending_confirmation");
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    let app = spawn_app().await;
    let email = format!("{}@example.com", uuid::Uuid::new_v4());
    let body = format!("name=le%20guin&email={}", email.replace('@', "%40"));

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(200, response.status().as_u16());

    let email_request = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = app.get_confirmation_links(&email_request);
    let html_link = confirmation_links.html.as_str();
    let text_link = confirmation_links.plain_text.as_str();

    assert_eq!(html_link, text_link);
}

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    let app = spawn_app().await;
    let body = format!("name=le%20guin&email={}", uuid::Uuid::new_v4());

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(400, response.status().as_u16());
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let app = spawn_app().await;
    let email = format!("{}@example.com", uuid::Uuid::new_v4());
    let body = format!("name=le%20guin&email={}", email.replace('@', "%40"));

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body).await;
    assert_eq!(200, response.status().as_u16());

    let email_request = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = app.get_confirmation_links(&email_request);

    let response = reqwest::get(confirmation_links.html)
        .await
        .unwrap();

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn clicking_on_the_subscriber_link_confirms_a_subscriber() {
    let app = spawn_app().await;
    let email = format!("{}@example.com", uuid::Uuid::new_v4());
    let body = format!("name=le%20guin&email={}", email.replace('@', "%40"));

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(200, response.status().as_u16());
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    let response = reqwest::get(confirmation_links.html)
    .await
    .unwrap()
    .error_for_status()
    .unwrap();

    let saved = sqlx::query!(
        "SELECT email, name, status FROM subscriptions WHERE email = $1",
        email
    )
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, email);
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "confirmed");
}
