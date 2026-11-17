use fpa_proto::v1 as pb;
use serde_json::Value;
pub fn text(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().trim().into()
}
pub fn uint(v: &Value, k: &str) -> Option<u32> {
    v[k].as_str()?.trim().parse().ok()
}
pub fn int(v: &Value, k: &str) -> Option<i32> {
    v[k].as_str()?.trim().parse().ok()
}
pub fn fixture(v: &Value, detail: bool) -> pb::Match {
    pb::Match {
        id: text(v, "ID_PROVA_JOGO"),
        number: text(v, "JOG_NUM_JOGO"),
        date: text(v, "JOG_DATA"),
        time: text(v, "JOG_HORA"),
        home: text(v, "EQUIPA_CASA"),
        away: text(v, "EQUIPA_FORA"),
        home_goals: uint(
            v,
            if detail {
                "GOLOS_CASA"
            } else {
                "JOG_GOLOS_CASA"
            },
        ),
        away_goals: uint(
            v,
            if detail {
                "GOLOS_FORA"
            } else {
                "JOG_GOLOS_FORA"
            },
        ),
        venue: text(v, if detail { "LOCAL" } else { "REC_NOME" }),
        round: text(v, "PJO_NUM_JORNADA"),
        home_club_id: text(v, "CLUBE_CASA_ID"),
        away_club_id: text(v, "CLUBE_FORA_ID"),
        source_state: if detail {
            text(v, "ESTADO")
        } else {
            String::new()
        },
    }
}
pub fn standing(v: &Value) -> pb::Standing {
    pb::Standing {
        team_id: text(v, "EQUIPA_ID"),
        name: text(v, "CLU_NOME"),
        position: int(v, "PEQ_CLASS_CLASSIFICACAO"),
        points: int(v, "PEQ_CLASS_PONTOS"),
        played: uint(v, "PEQ_CLASS_NUMERO_JOGOS"),
        wins: uint(v, "PEQ_CLASS_NUMERO_VITORIAS"),
        draws: uint(v, "PEQ_CLASS_NUMERO_EMPATES"),
        losses: uint(v, "PEQ_CLASS_NUMERO_DERROTAS"),
        goals_for: uint(v, "PEQ_CLASS_GOLOS_MARCADOS"),
        goals_against: uint(v, "PEQ_CLASS_GOLOS_SOFRIDOS"),
        difference: int(v, "DIFERENCA"),
    }
}
pub fn participant(v: &Value) -> pb::Participant {
    pb::Participant {
        registration_number: text(v, "CIP_NUMERO"),
        name: text(v, "NOME"),
        shirt: text(v, "PJP_NUM_CAMISOLA"),
        goals: uint(v, "PJP_GOLOS"),
        role: text(v, "FUNCAO"),
        yellow: text(v, "PJP_CARTAO_AMARELO_TISN"),
        exclusions: (1..=3)
            .filter(|n| text(v, &format!("PJP_2MIN_EXCLUSAO{n}_TISN")) == "S")
            .count() as u32,
        disqualified: text(v, "PJP_DESQUALIFICACAO_TISN"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn null_zero_negative_and_state() {
        let r = fixture(
            &json!({"JOG_GOLOS_CASA":"0","JOG_GOLOS_FORA":null,"JOGO_ESTADO_CODIGO":"50"}),
            false,
        );
        assert_eq!(r.home_goals, Some(0));
        assert_eq!(r.away_goals, None);
        assert!(r.source_state.is_empty());
        assert_eq!(uint(&json!({"n":"-1"}), "n"), None);
        assert_eq!(int(&json!({"n":"-7"}), "n"), Some(-7));
    }
}
