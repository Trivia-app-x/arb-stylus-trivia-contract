#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![cfg_attr(not(any(test, feature = "export-abi")), no_std)]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;
use stylus_sdk::{
    alloy_primitives::{Address, FixedBytes, U256, U8},
    alloy_sol_types::sol,
    prelude::*,
};

sol_storage! {
    #[entrypoint]
    pub struct TriviaChain {
        mapping(uint256 => GameSession) sessions;
        uint256 next_session_id;
        address owner;
    }

    pub struct GameSession {
        uint256 session_id;
        address host;
        bytes32 room_code;
        uint8 status; // 0: Created, 1: Active, 2: Completed
        uint256 start_time;
        uint256 current_question_index;
        uint256 question_start_time;
        uint256 question_duration;
        mapping(address => Player) players;
        address[] player_list;
        uint256 player_count;
        uint256 max_players;
        address winner;
        uint256 winning_score;
    }

    pub struct Player {
        address player_address;
        bytes32 display_name;
        uint256 score;
        uint256 current_streak;
        uint256 correct_answers;
        bool is_active;
    }
}

#[derive(SolidityError, Debug)]
pub enum TriviaChainError {
    Unauthorized(Unauthorized),
    SessionNotFound(SessionNotFound),
    SessionAlreadyActive(SessionAlreadyActive),
    SessionNotActive(SessionNotActive),
    SessionFull(SessionFull),
    PlayerNotInSession(PlayerNotInSession),
    PlayerAlreadyJoined(PlayerAlreadyJoined),
    InvalidRoomCode(InvalidRoomCode),
    AlreadyAnswered(AlreadyAnswered),
}

sol! {
    #[derive(Debug)]
    error Unauthorized();
    #[derive(Debug)]
    error SessionNotFound();
    #[derive(Debug)]
    error SessionAlreadyActive();
    #[derive(Debug)]
    error SessionNotActive();
    #[derive(Debug)]
    error SessionFull();
    #[derive(Debug)]
    error PlayerNotInSession();
    #[derive(Debug)]
    error PlayerAlreadyJoined();
    #[derive(Debug)]
    error InvalidRoomCode();
    #[derive(Debug)]
    error AlreadyAnswered();

    event SessionCreated(
        uint256 indexed sessionId,
        address indexed host,
        bytes32 roomCode,
        uint256 maxPlayers,
        uint64 timestamp
    );

    event PlayerJoined(
        uint256 indexed sessionId,
        address indexed player,
        uint256 playerCount
    );

    event SessionStarted(
        uint256 indexed sessionId,
        address indexed host,
        uint64 startTime
    );

    event SessionEnded(
        uint256 indexed sessionId,
        address indexed winner,
        uint256 winningScore,
        uint256 totalPlayers,
        uint64 endTime
    );

    event FinalScoreSubmitted(
        uint256 indexed sessionId,
        address indexed player,
        uint256 score,
        uint256 correctAnswers
    );
}

#[public]
impl TriviaChain {
    pub fn initialize(&mut self) -> Result<(), TriviaChainError> {
        if self.owner.get() != Address::ZERO {
            return Err(TriviaChainError::Unauthorized(Unauthorized {}));
        }
        self.owner.set(self.vm().msg_sender());
        self.next_session_id.set(U256::from(1));
        Ok(())
    }

    pub fn create_session(
        &mut self,
        room_code: FixedBytes<32>,
        max_players: U256,
        question_duration: U256,
    ) -> Result<U256, TriviaChainError> {
        let session_id = self.next_session_id.get();
        let session_host = self.vm().msg_sender();
        let session_timestamp = self.vm().block_timestamp();
        let mut session = self.sessions.setter(session_id);

        session.session_id.set(session_id);
        session.host.set(session_host);
        session.room_code.set(room_code);
        session.status.set(U8::from(0)); // Created
        session.max_players.set(max_players);
        session.question_duration.set(question_duration);
        session.player_count.set(U256::ZERO);
        session.current_question_index.set(U256::ZERO);

        self.next_session_id.set(session_id + U256::from(1));

        log(
            self.vm(),
            SessionCreated {
                sessionId: session_id,
                host: session_host,
                roomCode: room_code,
                maxPlayers: max_players,
                timestamp: session_timestamp,
            },
        );

        Ok(session_id)
    }

    pub fn join_session(
        &mut self,
        session_id: U256,
        room_code: FixedBytes<32>,
        display_name: FixedBytes<32>,
    ) -> Result<(), TriviaChainError> {
        let player_address = self.vm().msg_sender();

        let session = self.sessions.getter(session_id);
        let room_code_check = session.room_code.get();
        let status_check = session.status.get();
        let player_count_check = session.player_count.get();
        let max_players_check = session.max_players.get();
        let is_active_check = session.players.getter(player_address).is_active.get();

        if room_code_check != room_code {
            return Err(TriviaChainError::InvalidRoomCode(InvalidRoomCode {}));
        }

        if status_check != U8::from(0) {
            return Err(TriviaChainError::SessionAlreadyActive(
                SessionAlreadyActive {},
            ));
        }

        if player_count_check >= max_players_check {
            return Err(TriviaChainError::SessionFull(SessionFull {}));
        }

        if is_active_check {
            return Err(TriviaChainError::PlayerAlreadyJoined(
                PlayerAlreadyJoined {},
            ));
        }

        let mut session_mut = self.sessions.setter(session_id);
        let mut player = session_mut.players.setter(player_address);

        player.player_address.set(player_address);
        player.display_name.set(display_name);
        player.score.set(U256::ZERO);
        player.current_streak.set(U256::ZERO);
        player.correct_answers.set(U256::ZERO);
        player.is_active.set(true);

        session_mut.player_list.push(player_address);
        let new_player_count = player_count_check + U256::from(1);
        session_mut.player_count.set(new_player_count);

        log(
            self.vm(),
            PlayerJoined {
                sessionId: session_id,
                player: player_address,
                playerCount: new_player_count,
            },
        );

        Ok(())
    }

    pub fn start_session(&mut self, session_id: U256) -> Result<(), TriviaChainError> {
        let session = self.sessions.getter(session_id);
        let session_host = self.vm().msg_sender();
        let session_timestamp = self.vm().block_timestamp();

        if session.host.get() != session_host {
            return Err(TriviaChainError::Unauthorized(Unauthorized {}));
        }

        if session.status.get() != U8::from(0) {
            return Err(TriviaChainError::SessionAlreadyActive(
                SessionAlreadyActive {},
            ));
        }

        let mut session_mut = self.sessions.setter(session_id);
        session_mut.status.set(U8::from(1)); // Active
        session_mut.start_time.set(U256::from(session_timestamp));

        // Emit SessionStarted event
        log(
            self.vm(),
            SessionStarted {
                sessionId: session_id,
                host: session_host,
                startTime: session_timestamp,
            },
        );

        Ok(())
    }

    pub fn start_question(
        &mut self,
        session_id: U256,
        question_index: U256,
    ) -> Result<(), TriviaChainError> {
        let session_host = self.vm().msg_sender();
        let session_timestamp = self.vm().block_timestamp();

        let session = self.sessions.getter(session_id);
        let host_check = session.host.get();
        let status_check = session.status.get();

        if host_check != session_host {
            return Err(TriviaChainError::Unauthorized(Unauthorized {}));
        }

        if status_check != U8::from(1) {
            return Err(TriviaChainError::SessionNotActive(SessionNotActive {}));
        }

        let mut session_mut = self.sessions.setter(session_id);
        session_mut.current_question_index.set(question_index);
        session_mut
            .question_start_time
            .set(U256::from(session_timestamp));

        Ok(())
    }

    pub fn submit_final_score(
        &mut self,
        session_id: U256,
        total_score: U256,
        correct_answers: U256,
    ) -> Result<(), TriviaChainError> {
        let player_address = self.vm().msg_sender();

        let session = self.sessions.getter(session_id);
        let status_check = session.status.get();

        // Check session is ended (status = 2)
        if status_check != U8::from(2) {
            return Err(TriviaChainError::SessionNotActive(SessionNotActive {}));
        }

        let player = session.players.getter(player_address);
        let is_active_check = player.is_active.get();
        let current_score = player.score.get();

        if !is_active_check {
            return Err(TriviaChainError::PlayerNotInSession(PlayerNotInSession {}));
        }

        // Only allow one final submission per player
        if current_score > U256::ZERO {
            return Err(TriviaChainError::AlreadyAnswered(AlreadyAnswered {}));
        }

        // Update player's final score
        let mut session_mut = self.sessions.setter(session_id);
        let mut player_mut = session_mut.players.setter(player_address);

        player_mut.score.set(total_score);
        player_mut.correct_answers.set(correct_answers);

        // Update winner if this player has highest score
        let current_winning_score = session_mut.winning_score.get();
        if total_score > current_winning_score {
            session_mut.winner.set(player_address);
            session_mut.winning_score.set(total_score);
        }

        log(
            self.vm(),
            FinalScoreSubmitted {
                sessionId: session_id,
                player: player_address,
                score: total_score,
                correctAnswers: correct_answers,
            },
        );

        Ok(())
    }

    // Simplified end_session - no loops!
    pub fn end_session(&mut self, session_id: U256) -> Result<Address, TriviaChainError> {
        let session_timestamp = self.vm().block_timestamp();
        let session = self.sessions.getter(session_id);
        let host_check = session.host.get();
        let status_check = session.status.get();

        if host_check != self.vm().msg_sender() {
            return Err(TriviaChainError::Unauthorized(Unauthorized {}));
        }

        if status_check != U8::from(1) {
            return Err(TriviaChainError::SessionNotActive(SessionNotActive {}));
        }

        // Winner is already tracked during gameplay
        let winner_address = session.winner.get();
        let winning_score = session.winning_score.get();
        let player_count = session.player_count.get();

        let mut session_mut = self.sessions.setter(session_id);
        session_mut.status.set(U8::from(2)); // Completed

        log(
            self.vm(),
            SessionEnded {
                sessionId: session_id,
                winner: winner_address,
                winningScore: winning_score,
                totalPlayers: player_count,
                endTime: session_timestamp,
            },
        );

        Ok(winner_address)
    }

    // View functions
    pub fn get_winner(&self, session_id: U256) -> Address {
        self.sessions.getter(session_id).winner.get()
    }

    pub fn get_player_score(&self, session_id: U256, player: Address) -> U256 {
        self.sessions
            .getter(session_id)
            .players
            .getter(player)
            .score
            .get()
    }
}
