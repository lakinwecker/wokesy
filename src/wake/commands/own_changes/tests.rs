use std::time::Duration;

use tokio::sync::watch;
use wiremock::ResponseTemplate;

use super::*;
use crate::on_call::Roster;
use crate::wake::testing::{
    ALICE, Answering, BOB, FISHNET_OVERRIDES, PRIMARY_OVERRIDES, PRIMARY_SHIFTS, STRANGER, Setup,
    directory, in_sysadmin, lichess_alone,
};

const NOTHING_OF_THE_SENDER: &str = "I have no override or shift of yours on record. I only end the ones I made myself, and I forget them when I restart. Anything I made earlier still ends on time by itself, or you can remove it in PagerDuty.";
const NOTHING_AT_ALL: &str = "Nothing to undo: I have no override or shift of mine on record. I forget them when I restart; anything I made earlier still ends on time by itself.";
const SPARE_OVERRIDES: &str = "/schedules/PSPARE1/overrides";
const INTERNAL_ERROR: &str =
    "pagerduty rejected the request (500 Internal Server Error): Internal Server Error";

async fn woken(answering: &mut Answering, sender: ChatUserId) {
    let said = answering
        .said_either_way(&in_sysadmin(sender, "wake me for 4 hours"))
        .await;
    assert!(said.starts_with(":alarm_clock:"), "{said}");
    assert!(answering.was_asked_to_refresh().await);
}

async fn alice_and_bob_woken_on_both_rotas() -> Answering {
    let mut answering = Setup::waking_on_both_rotas().start().await;
    answering
        .creates_overrides(FISHNET_OVERRIDES, &["PQALICE", "PQBOB00"])
        .await;
    answering
        .creates_overrides(PRIMARY_OVERRIDES, &["PRALICE", "PRBOB00"])
        .await;
    woken(&mut answering, ALICE).await;
    woken(&mut answering, BOB).await;
    answering
}

async fn alice_woken_on_the_primary_rota() -> Answering {
    let mut answering = Setup::default().start().await;
    answering
        .creates_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;
    woken(&mut answering, ALICE).await;
    answering
}

async fn alice_woken_on_the_legacy_fishnet_rota_and_the_shift_based_primary_rota() -> Answering {
    let mut answering = Setup::waking_on_both_rotas().start().await;
    answering
        .creates_overrides(FISHNET_OVERRIDES, &["PQALICE"])
        .await;
    answering.is_shift_based(PRIMARY_OVERRIDES).await;
    answering
        .creates_custom_shifts(PRIMARY_SHIFTS, &["PCALICE"])
        .await;
    woken(&mut answering, ALICE).await;
    answering
}

async fn alice_woken_on_the_shift_based_primary_rota() -> Answering {
    let mut answering = Setup::default().start().await;
    answering.is_shift_based(PRIMARY_OVERRIDES).await;
    answering
        .creates_custom_shifts(PRIMARY_SHIFTS, &["PCALICE"])
        .await;
    woken(&mut answering, ALICE).await;
    answering
}

async fn requests_after_waking(answering: &Answering) -> Vec<String> {
    answering
        .requests()
        .await
        .into_iter()
        .filter(|request| !request.starts_with("POST "))
        .collect()
}

fn delete(overrides: &str, id: &str) -> String {
    format!("DELETE {overrides}/{id}")
}

#[tokio::test]
async fn stop_ends_only_the_overrides_made_for_the_sender_by_their_ids() {
    let mut answering = alice_and_bob_woken_on_both_rotas().await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;

    let otherwise = answering
        .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
        .await;

    assert_eq!(
        otherwise,
        ":zzz: Ended your override on **Fishnet rota**, **Primary rota**. The schedule decides again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(FISHNET_OVERRIDES, "PQALICE"),
            delete(PRIMARY_OVERRIDES, "PRALICE")
        ]
    );
    assert!(answering.was_asked_to_refresh().await);
}

#[tokio::test]
async fn after_someone_else_stopped_the_senders_own_overrides_are_still_on_record() {
    let mut answering = alice_and_bob_woken_on_both_rotas().await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE", "PQBOB00"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE", "PRBOB00"])
        .await;
    answering
        .answer(&in_sysadmin(ALICE, "stop waking me"))
        .await;

    let otherwise = answering.acknowledged(&in_sysadmin(BOB, "stop")).await;

    assert_eq!(
        otherwise,
        ":zzz: Ended your override on **Fishnet rota**, **Primary rota**. The schedule decides again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(FISHNET_OVERRIDES, "PQALICE"),
            delete(PRIMARY_OVERRIDES, "PRALICE"),
            delete(FISHNET_OVERRIDES, "PQBOB00"),
            delete(PRIMARY_OVERRIDES, "PRBOB00"),
        ]
    );
}

#[tokio::test]
async fn an_ended_override_is_forgotten() {
    let mut answering = alice_woken_on_the_primary_rota().await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;
    answering
        .answer(&in_sysadmin(ALICE, "stop waking me"))
        .await;
    assert!(answering.was_asked_to_refresh().await);

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        NOTHING_OF_THE_SENDER
    );
    assert_eq!(
        answering
            .reply(&in_sysadmin(ALICE, "back to defaults"))
            .await,
        NOTHING_AT_ALL
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [delete(PRIMARY_OVERRIDES, "PRALICE")]
    );
    assert!(!answering.was_asked_to_refresh().await);
}

#[tokio::test]
async fn every_time_the_sender_was_woken_is_ended() {
    let mut answering = Setup::default().start().await;
    answering
        .creates_overrides(PRIMARY_OVERRIDES, &["PRFIRST", "PRSECND"])
        .await;
    woken(&mut answering, ALICE).await;
    woken(&mut answering, ALICE).await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRFIRST", "PRSECND"])
        .await;

    let otherwise = answering
        .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
        .await;

    assert_eq!(
        otherwise,
        ":zzz: Ended your override on **Primary rota**. The schedule decides again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(PRIMARY_OVERRIDES, "PRFIRST"),
            delete(PRIMARY_OVERRIDES, "PRSECND")
        ]
    );
}

#[tokio::test]
async fn reset_ends_every_override_on_record_whoever_it_is_for() {
    let mut answering = alice_and_bob_woken_on_both_rotas().await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE", "PQBOB00"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE", "PRBOB00"])
        .await;

    let otherwise = answering
        .acknowledged(&in_sysadmin(BOB, "back to defaults"))
        .await;

    assert_eq!(
        otherwise,
        ":leftwards_arrow_with_hook: Back to the defaults.\n\
         - **Fishnet rota**: ended the override of Alice Example, Bob Example.\n\
         - **Primary rota**: ended the override of Alice Example, Bob Example.\n\
         PagerDuty's own schedules and overrides decide again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(FISHNET_OVERRIDES, "PQALICE"),
            delete(PRIMARY_OVERRIDES, "PRALICE"),
            delete(FISHNET_OVERRIDES, "PQBOB00"),
            delete(PRIMARY_OVERRIDES, "PRBOB00"),
        ]
    );
    assert!(answering.was_asked_to_refresh().await);
    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        NOTHING_OF_THE_SENDER
    );
}

#[tokio::test]
async fn reset_is_refused_to_a_sender_who_is_no_known_pagerduty_user() {
    let mut answering = alice_woken_on_the_primary_rota().await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;

    assert_eq!(
        answering
            .reply(&in_sysadmin(STRANGER, "back to defaults"))
            .await,
        "I don't know which PagerDuty user you are. I match people by email address, then by full name. Ask whoever runs wokesy to link your accounts."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        Vec::<String>::new()
    );
    assert!(!answering.was_asked_to_refresh().await);

    assert_eq!(
        answering
            .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
            .await,
        ":zzz: Ended your override on **Primary rota**. The schedule decides again."
    );
}

#[tokio::test]
async fn stop_from_a_sender_who_is_no_known_pagerduty_user_ends_nothing() {
    let mut answering = alice_woken_on_the_primary_rota().await;

    assert_eq!(
        answering
            .reply(&in_sysadmin(STRANGER, "stop waking me"))
            .await,
        NOTHING_OF_THE_SENDER
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn reset_names_by_their_pagerduty_id_a_person_the_directory_no_longer_knows() {
    let mut answering = alice_woken_on_the_primary_rota().await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;
    answering
        .directory
        .send_replace(Some(directory(&[("PBOB001", "Bob Example", Some(BOB))])));

    assert_eq!(
        answering.acknowledged(&in_sysadmin(BOB, "reset")).await,
        ":leftwards_arrow_with_hook: Back to the defaults.\n\
         - **Primary rota**: ended the override of PALICE1.\n\
         PagerDuty's own schedules and overrides decide again."
    );
}

#[tokio::test]
async fn an_override_someone_already_removed_in_pagerduty_counts_as_ended() {
    for (command, reply) in [
        (
            "stop waking me",
            ":zzz: Ended your override on **Primary rota**. The schedule decides again.",
        ),
        (
            "back to defaults",
            ":leftwards_arrow_with_hook: Back to the defaults.\n\
             - **Primary rota**: ended the override of Alice Example.\n\
             PagerDuty's own schedules and overrides decide again.",
        ),
    ] {
        let mut answering = alice_woken_on_the_primary_rota().await;
        answering
            .answers_a_delete(PRIMARY_OVERRIDES, "PRALICE", ResponseTemplate::new(404))
            .await;

        assert_eq!(
            answering.acknowledged(&in_sysadmin(ALICE, command)).await,
            reply
        );
        assert_eq!(
            answering.reply(&in_sysadmin(ALICE, command)).await,
            if command.starts_with("stop") {
                NOTHING_OF_THE_SENDER
            } else {
                NOTHING_AT_ALL
            }
        );
        assert_eq!(
            requests_after_waking(&answering).await,
            [delete(PRIMARY_OVERRIDES, "PRALICE")]
        );
    }
}

#[tokio::test]
async fn an_override_that_could_not_be_ended_stays_on_record_so_stop_can_be_tried_again() {
    let mut answering = alice_and_bob_woken_on_both_rotas().await;
    answering
        .answers_a_delete(FISHNET_OVERRIDES, "PQALICE", ResponseTemplate::new(500))
        .await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        format!(
            ":zzz: Ended your override on **Primary rota**. The schedule decides again.\n\
             I couldn't end your override on **Fishnet rota**: {INTERNAL_ERROR}"
        )
    );
    assert!(answering.was_asked_to_refresh().await);
    assert_eq!(
        answering
            .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
            .await,
        ":zzz: Ended your override on **Fishnet rota**. The schedule decides again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(FISHNET_OVERRIDES, "PQALICE"),
            delete(PRIMARY_OVERRIDES, "PRALICE"),
            delete(FISHNET_OVERRIDES, "PQALICE"),
        ]
    );
}

#[tokio::test]
async fn when_no_override_could_be_ended_stop_reports_only_the_failures() {
    let mut answering = alice_woken_on_the_primary_rota().await;
    answering
        .answers_a_delete(PRIMARY_OVERRIDES, "PRALICE", ResponseTemplate::new(500))
        .await;

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        format!("I couldn't end your override on **Primary rota**: {INTERNAL_ERROR}")
    );
}

#[tokio::test]
async fn an_override_that_could_not_be_ended_stays_on_record_so_reset_can_be_tried_again() {
    let mut answering = alice_and_bob_woken_on_both_rotas().await;
    answering
        .answers_a_delete(FISHNET_OVERRIDES, "PQBOB00", ResponseTemplate::new(500))
        .await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE", "PQBOB00"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE", "PRBOB00"])
        .await;

    assert_eq!(
        answering
            .reply(&in_sysadmin(ALICE, "back to defaults"))
            .await,
        format!(
            ":leftwards_arrow_with_hook: Back to the defaults.\n\
             - **Fishnet rota**: ended the override of Alice Example. I couldn't end the override of Bob Example: {INTERNAL_ERROR}\n\
             - **Primary rota**: ended the override of Alice Example, Bob Example.\n\
             PagerDuty's own schedules and overrides decide again."
        )
    );
    assert_eq!(
        answering
            .acknowledged(&in_sysadmin(ALICE, "back to defaults"))
            .await,
        ":leftwards_arrow_with_hook: Back to the defaults.\n\
         - **Fishnet rota**: ended the override of Bob Example.\n\
         PagerDuty's own schedules and overrides decide again."
    );
    assert_eq!(
        requests_after_waking(&answering).await.last().unwrap(),
        &delete(FISHNET_OVERRIDES, "PQBOB00")
    );
}

#[tokio::test]
async fn an_override_whose_time_is_up_is_forgotten_instead_of_ended() {
    for (command, reply) in [
        ("stop waking me", NOTHING_OF_THE_SENDER),
        ("back to defaults", NOTHING_AT_ALL),
    ] {
        let mut answering = alice_woken_on_the_primary_rota().await;
        tokio::time::pause();
        tokio::time::advance(Duration::from_hours(4)).await;
        tokio::time::resume();

        assert_eq!(answering.reply(&in_sysadmin(ALICE, command)).await, reply);
        assert_eq!(
            requests_after_waking(&answering).await,
            Vec::<String>::new()
        );
    }
}

#[tokio::test]
async fn an_override_is_still_ended_just_before_its_time_is_up() {
    let mut answering = alice_woken_on_the_primary_rota().await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_mins(239)).await;
    tokio::time::resume();

    assert_eq!(
        answering
            .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
            .await,
        ":zzz: Ended your override on **Primary rota**. The schedule decides again."
    );
}

#[tokio::test]
async fn an_override_that_was_refused_is_not_on_record() {
    let mut answering = Setup::default().start().await;
    answering.refuses_overrides(PRIMARY_OVERRIDES).await;
    answering
        .reply(&in_sysadmin(ALICE, "wake me for 4 hours"))
        .await;

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        NOTHING_OF_THE_SENDER
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn with_nothing_on_record_as_after_a_restart_stop_and_reset_ask_pagerduty_for_nothing() {
    let mut answering = Setup::waking_on_both_rotas().start().await;

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        NOTHING_OF_THE_SENDER
    );
    assert_eq!(
        answering
            .reply(&in_sysadmin(ALICE, "back to defaults"))
            .await,
        NOTHING_AT_ALL
    );
    assert_eq!(answering.requests().await, Vec::<String>::new());
    assert!(!answering.was_asked_to_refresh().await);
}

#[tokio::test]
async fn an_override_on_a_schedule_the_roster_does_not_know_is_ended_and_named_by_its_id() {
    for (command, reply) in [
        (
            "stop waking me",
            ":zzz: Ended your override on `PSPARE1`. The schedule decides again.",
        ),
        (
            "back to defaults",
            ":leftwards_arrow_with_hook: Back to the defaults.\n\
             - `PSPARE1`: ended the override of Alice Example.\n\
             PagerDuty's own schedules and overrides decide again.",
        ),
    ] {
        let mut answering = Setup::waking_on("PSPARE1", lichess_alone()).start().await;
        answering
            .creates_overrides(SPARE_OVERRIDES, &["PSALICE"])
            .await;
        woken(&mut answering, ALICE).await;
        answering
            .deletes_overrides(SPARE_OVERRIDES, &["PSALICE"])
            .await;

        assert_eq!(
            answering.acknowledged(&in_sysadmin(ALICE, command)).await,
            reply
        );
        assert_eq!(
            requests_after_waking(&answering).await,
            [delete(SPARE_OVERRIDES, "PSALICE")]
        );
    }
}

#[tokio::test]
async fn overrides_made_before_the_roster_knew_the_schedule_and_after_are_ended_on_one_line() {
    let mut answering = Setup {
        roster: Some(Roster { policies: vec![] }),
        ..Setup::default()
    }
    .start()
    .await;
    answering
        .creates_overrides(PRIMARY_OVERRIDES, &["PRALICE", "PRBOB00"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE", "PRBOB00"])
        .await;
    woken(&mut answering, ALICE).await;
    answering.roster.send_replace(Some(lichess_alone()));
    woken(&mut answering, BOB).await;

    assert_eq!(
        answering
            .acknowledged(&in_sysadmin(ALICE, "back to defaults"))
            .await,
        ":leftwards_arrow_with_hook: Back to the defaults.\n\
         - `PI7DH85`: ended the override of Alice Example, Bob Example.\n\
         PagerDuty's own schedules and overrides decide again."
    );
}

#[tokio::test]
async fn stop_ends_an_override_through_the_v2_path_and_a_custom_shift_through_the_v3_path() {
    let mut answering =
        alice_woken_on_the_legacy_fishnet_rota_and_the_shift_based_primary_rota().await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE"])
        .await;
    answering
        .answers_a_delete(PRIMARY_SHIFTS, "PCALICE", ResponseTemplate::new(204))
        .await;

    let otherwise = answering
        .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
        .await;

    assert_eq!(
        otherwise,
        ":zzz: Ended your override on **Fishnet rota**. The schedule decides again.\n\
         :zzz: Ended your shift on **Primary rota**. The schedule decides again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(FISHNET_OVERRIDES, "PQALICE"),
            delete(PRIMARY_SHIFTS, "PCALICE")
        ]
    );
    assert!(answering.was_asked_to_refresh().await);
    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        NOTHING_OF_THE_SENDER
    );
}

#[tokio::test]
async fn reset_ends_an_override_through_the_v2_path_and_a_custom_shift_through_the_v3_path() {
    let mut answering =
        alice_woken_on_the_legacy_fishnet_rota_and_the_shift_based_primary_rota().await;
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE"])
        .await;
    answering
        .answers_a_delete(PRIMARY_SHIFTS, "PCALICE", ResponseTemplate::new(204))
        .await;

    let otherwise = answering
        .acknowledged(&in_sysadmin(BOB, "back to defaults"))
        .await;

    assert_eq!(
        otherwise,
        ":leftwards_arrow_with_hook: Back to the defaults.\n\
         - **Fishnet rota**: ended the override of Alice Example.\n\
         - **Primary rota**: ended the shift of Alice Example.\n\
         PagerDuty's own schedules and overrides decide again."
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        [
            delete(FISHNET_OVERRIDES, "PQALICE"),
            delete(PRIMARY_SHIFTS, "PCALICE")
        ]
    );
    assert_eq!(
        answering.reply(&in_sysadmin(BOB, "back to defaults")).await,
        NOTHING_AT_ALL
    );
}

#[tokio::test]
async fn a_custom_shift_that_is_gone_or_has_already_ended_counts_as_ended() {
    for status in [404, 400] {
        for (command, reply, nothing) in [
            (
                "stop waking me",
                ":zzz: Ended your shift on **Primary rota**. The schedule decides again.",
                NOTHING_OF_THE_SENDER,
            ),
            (
                "back to defaults",
                ":leftwards_arrow_with_hook: Back to the defaults.\n\
                 - **Primary rota**: ended the shift of Alice Example.\n\
                 PagerDuty's own schedules and overrides decide again.",
                NOTHING_AT_ALL,
            ),
        ] {
            let mut answering = alice_woken_on_the_shift_based_primary_rota().await;
            answering
                .answers_a_delete(PRIMARY_SHIFTS, "PCALICE", ResponseTemplate::new(status))
                .await;

            assert_eq!(
                answering.acknowledged(&in_sysadmin(ALICE, command)).await,
                reply,
                "{status}"
            );
            assert_eq!(
                answering.reply(&in_sysadmin(ALICE, command)).await,
                nothing,
                "{status}"
            );
            assert_eq!(
                requests_after_waking(&answering).await,
                [delete(PRIMARY_SHIFTS, "PCALICE")]
            );
        }
    }
}

#[tokio::test]
async fn a_custom_shift_that_could_not_be_ended_stays_on_record_so_it_can_be_tried_again() {
    for (command, failed, ended) in [
        (
            "stop waking me",
            format!("I couldn't end your shift on **Primary rota**: {INTERNAL_ERROR}"),
            ":zzz: Ended your shift on **Primary rota**. The schedule decides again.",
        ),
        (
            "back to defaults",
            format!(
                ":leftwards_arrow_with_hook: Back to the defaults.\n\
                 - **Primary rota**: I couldn't end the shift of Alice Example: {INTERNAL_ERROR}\n\
                 PagerDuty's own schedules and overrides decide again."
            ),
            ":leftwards_arrow_with_hook: Back to the defaults.\n\
             - **Primary rota**: ended the shift of Alice Example.\n\
             PagerDuty's own schedules and overrides decide again.",
        ),
    ] {
        let mut answering = alice_woken_on_the_shift_based_primary_rota().await;
        answering
            .answers_a_delete(PRIMARY_SHIFTS, "PCALICE", ResponseTemplate::new(500))
            .await;
        answering
            .answers_a_delete(PRIMARY_SHIFTS, "PCALICE", ResponseTemplate::new(204))
            .await;

        assert_eq!(answering.reply(&in_sysadmin(ALICE, command)).await, failed);
        assert_eq!(
            answering.acknowledged(&in_sysadmin(ALICE, command)).await,
            ended
        );
        assert_eq!(
            requests_after_waking(&answering).await,
            [
                delete(PRIMARY_SHIFTS, "PCALICE"),
                delete(PRIMARY_SHIFTS, "PCALICE")
            ]
        );
    }
}

#[tokio::test]
async fn an_override_whose_delete_is_answered_400_is_not_ended_and_stays_on_record() {
    let mut answering = alice_woken_on_the_primary_rota().await;
    answering
        .answers_a_delete(PRIMARY_OVERRIDES, "PRALICE", ResponseTemplate::new(400))
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        "I couldn't end your override on **Primary rota**: pagerduty rejected the request (400 Bad Request): Bad Request"
    );
    assert_eq!(
        answering
            .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
            .await,
        ":zzz: Ended your override on **Primary rota**. The schedule decides again."
    );
}

#[tokio::test]
async fn a_custom_shift_that_was_refused_is_not_on_record() {
    let mut answering = Setup::default().start().await;
    answering.is_shift_based(PRIMARY_OVERRIDES).await;
    answering.refuses_overrides(PRIMARY_SHIFTS).await;
    answering
        .reply(&in_sysadmin(ALICE, "wake me for 4 hours"))
        .await;

    assert_eq!(
        answering.reply(&in_sysadmin(ALICE, "stop waking me")).await,
        NOTHING_OF_THE_SENDER
    );
    assert_eq!(
        requests_after_waking(&answering).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn who_asked_in_which_message_is_told_for_each_change_on_record_and_withdrawn_once_it_ended()
{
    let mut answering = alice_and_bob_woken_on_both_rotas().await;
    let asked = answering.commands.asked();
    let told = |asked: &watch::Receiver<Vec<Asked>>| -> Vec<(String, String, String)> {
        asked
            .borrow()
            .iter()
            .map(|asked| {
                (
                    asked.user.to_string(),
                    asked.schedule.to_string(),
                    asked.in_message.in_chat(),
                )
            })
            .collect()
    };
    let from = || "#**sysadmin>incident@1**".to_owned();
    let told_before = told(&asked);
    answering
        .deletes_overrides(FISHNET_OVERRIDES, &["PQALICE"])
        .await;
    answering
        .deletes_overrides(PRIMARY_OVERRIDES, &["PRALICE"])
        .await;

    answering
        .acknowledged(&in_sysadmin(ALICE, "stop waking me"))
        .await;

    assert_eq!(
        told_before,
        [
            ("PALICE1".to_owned(), "PFISHNT".to_owned(), from()),
            ("PALICE1".to_owned(), "PI7DH85".to_owned(), from()),
            ("PBOB001".to_owned(), "PFISHNT".to_owned(), from()),
            ("PBOB001".to_owned(), "PI7DH85".to_owned(), from()),
        ]
    );
    assert_eq!(
        told(&asked),
        [
            ("PBOB001".to_owned(), "PFISHNT".to_owned(), from()),
            ("PBOB001".to_owned(), "PI7DH85".to_owned(), from()),
        ]
    );
}
