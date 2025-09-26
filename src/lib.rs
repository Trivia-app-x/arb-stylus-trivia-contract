#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![cfg_attr(not(any(test, feature = "export-abi")), no_std)]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;
use stylus_sdk::{
    alloy_primitives::{U256, U8, Address, FixedBytes},
    alloy_sol_types::sol,
    prelude::*,
};

sol_storage! {
    #[entrypoint]
    pub struct TriviaChain {
        mapping(uint256 => GameSession) sessions;
        mapping(address => PlayerStats) player_stats;
        uint256 next_session_id;
        uint256 active_sessions_count;
        address owner;
    }

    pub struct GameSession {
        uint256 session_id;
        address host;
        bytes32 room_code;
        uint8 status; // 0: Created, 1: Active, 2: Completed, 3: Cancelled
        uint256 start_time;
        uint256 end_time;
        uint256 question_count;
        uint256 current_question_index;
        uint256 question_start_time;
        uint256 question_duration; // in seconds
        mapping(address => Player) players;
        address[] player_list;
        uint256 player_count;
        uint256 max_players;
        uint256 prize_pool;
        address winner;
        mapping(uint256 => QuestionData) questions;
        mapping(uint256 => mapping(address => Answer)) answers;
    }

    pub struct Player {
        address player_address;
        bytes32 display_name;
        uint256 score;
        uint256 current_streak;
        uint256 best_streak;
        uint256 correct_answers;
        uint256 total_response_time; // cumulative response time in milliseconds
        bool is_active;
        uint256 join_time;
        uint8 rank;
    }

    pub struct PlayerStats {
        uint256 games_played;
        uint256 total_wins;
        uint256 total_score;
        uint256 best_score;
        uint256 total_correct_answers;
        uint256 longest_streak;
    }

    pub struct QuestionData {
        bytes32 question_hash; // Hash of the question for integrity
        uint8 question_type; // 0: Multiple choice, 1: True/False, 2: Numeric
        uint8 difficulty; // 0: Easy, 1: Medium, 2: Hard
        uint256 time_limit; // in seconds
        bytes32 correct_answer_hash; // Hash of correct answer
    }

    pub struct Answer {
        bytes32 answer_hash;
        uint256 submit_time; // timestamp when answer was submitted
        bool is_correct;
        uint256 points_earned;
    }
}

#[derive(SolidityError)]
pub enum TriviaChainError {
    Unauthorized(Unauthorized),
    SessionNotFound(SessionNotFound),
    SessionAlreadyActive(SessionAlreadyActive),
    SessionNotActive(SessionNotActive),
    SessionFull(SessionFull),
    PlayerNotInSession(PlayerNotInSession),
    PlayerAlreadyJoined(PlayerAlreadyJoined),
    InvalidRoomCode(InvalidRoomCode),
    InvalidQuestionIndex(InvalidQuestionIndex),
    QuestionNotActive(QuestionNotActive),
    AlreadyAnswered(AlreadyAnswered),
    InvalidAnswer(InvalidAnswer),
    InsufficientPrize(InsufficientPrize),
}

sol! {
    error Unauthorized();
    error SessionNotFound();
    error SessionAlreadyActive();
    error SessionNotActive();
    error SessionFull();
    error PlayerNotInSession();
    error PlayerAlreadyJoined();
    error InvalidRoomCode();
    error InvalidQuestionIndex();
    error QuestionNotActive();
    error AlreadyAnswered();
    error InvalidAnswer();
    error InsufficientPrize();
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
        self.active_sessions_count.set(self.active_sessions_count.get() + U256::from(1));

        Ok(session_id)
    }

    pub fn join_session(
        &mut self,
        session_id: U256,
        room_code: FixedBytes<32>,
        display_name: FixedBytes<32>,
    ) -> Result<(), TriviaChainError> {
        let player_address = self.vm().msg_sender();
        let session_timestamp = self.vm().block_timestamp();

        // Read all session data first
        let session = self.sessions.getter(session_id);
        let room_code_check = session.room_code.get();
        let status_check = session.status.get();
        let player_count_check = session.player_count.get();
        let max_players_check = session.max_players.get();
        let is_active_check = session.players.getter(player_address).is_active.get();

        // Perform all checks
        if room_code_check != room_code {
            return Err(TriviaChainError::InvalidRoomCode(InvalidRoomCode {}));
        }

        if status_check != U8::from(0) {
            return Err(TriviaChainError::SessionAlreadyActive(SessionAlreadyActive {}));
        }

        if player_count_check >= max_players_check {
            return Err(TriviaChainError::SessionFull(SessionFull {}));
        }

        if is_active_check {
            return Err(TriviaChainError::PlayerAlreadyJoined(PlayerAlreadyJoined {}));
        }

        // Now do mutations
        let mut session_mut = self.sessions.setter(session_id);
        let mut player = session_mut.players.setter(player_address);

        player.player_address.set(player_address);
        player.display_name.set(display_name);
        player.score.set(U256::ZERO);
        player.current_streak.set(U256::ZERO);
        player.best_streak.set(U256::ZERO);
        player.correct_answers.set(U256::ZERO);
        player.total_response_time.set(U256::ZERO);
        player.is_active.set(true);
        player.join_time.set(U256::from(session_timestamp));

        session_mut.player_list.push(player_address);
        session_mut.player_count.set(player_count_check + U256::from(1));

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
            return Err(TriviaChainError::SessionAlreadyActive(SessionAlreadyActive {}));
        }

        let mut session_mut = self.sessions.setter(session_id);
        session_mut.status.set(U8::from(1)); // Active
        session_mut.start_time.set(U256::from(session_timestamp));

        Ok(())
    }

    pub fn start_question(
        &mut self,
        session_id: U256,
        question_index: U256,
        question_hash: FixedBytes<32>,
        question_type: U8,
        difficulty: U8,
        correct_answer_hash: FixedBytes<32>,
    ) -> Result<(), TriviaChainError> {
        let session_host = self.vm().msg_sender();
        let session_timestamp = self.vm().block_timestamp();

        // Read all session data first
        let session = self.sessions.getter(session_id);
        let host_check = session.host.get();
        let status_check = session.status.get();
        let question_duration = session.question_duration.get();

        // Perform checks
        if host_check != session_host {
            return Err(TriviaChainError::Unauthorized(Unauthorized {}));
        }

        if status_check != U8::from(1) {
            return Err(TriviaChainError::SessionNotActive(SessionNotActive {}));
        }

        // Now do mutations
        let mut session_mut = self.sessions.setter(session_id);
        let mut question = session_mut.questions.setter(question_index);

        question.question_hash.set(question_hash);
        question.question_type.set(question_type);
        question.difficulty.set(difficulty);
        question.time_limit.set(question_duration);
        question.correct_answer_hash.set(correct_answer_hash);

        session_mut.current_question_index.set(question_index);
        session_mut.question_start_time.set(U256::from(session_timestamp));

        Ok(())
    }

    pub fn submit_answer(
        &mut self,
        session_id: U256,
        question_index: U256,
        answer_hash: FixedBytes<32>,
    ) -> Result<U256, TriviaChainError> {
        let player_address = self.vm().msg_sender();
        let session_timestamp = self.vm().block_timestamp();
        let current_time = U256::from(session_timestamp);

        // Read all session data first
        let session = self.sessions.getter(session_id);
        let status_check = session.status.get();
        let current_question_check = session.current_question_index.get();
        let question_start = session.question_start_time.get();
        let time_limit = session.question_duration.get();

        // Check session and question status
        if status_check != U8::from(1) {
            return Err(TriviaChainError::SessionNotActive(SessionNotActive {}));
        }

        if current_question_check != question_index {
            return Err(TriviaChainError::InvalidQuestionIndex(InvalidQuestionIndex {}));
        }

        // Read player data
        let player = session.players.getter(player_address);
        let is_active_check = player.is_active.get();
        let player_current_streak = player.current_streak.get();
        let player_best_streak = player.best_streak.get();
        let player_correct_answers = player.correct_answers.get();
        let player_score = player.score.get();
        let player_total_response_time = player.total_response_time.get();

        if !is_active_check {
            return Err(TriviaChainError::PlayerNotInSession(PlayerNotInSession {}));
        }

        // Check if already answered - need to handle the temporary value
        let answers_getter = session.answers.getter(question_index);
        let existing_submit_time = answers_getter.getter(player_address).submit_time.get();

        if existing_submit_time != U256::ZERO {
            return Err(TriviaChainError::AlreadyAnswered(AlreadyAnswered {}));
        }

        if current_time > question_start + time_limit {
            return Err(TriviaChainError::QuestionNotActive(QuestionNotActive {}));
        }

        // Read question data
        let question = session.questions.getter(question_index);
        let correct_answer_hash = question.correct_answer_hash.get();
        let difficulty = question.difficulty.get();

        let is_correct = answer_hash == correct_answer_hash;

        // Calculate points
        let response_time = current_time - question_start;
        let time_bonus = if response_time < time_limit {
            let bonus_ratio = (time_limit - response_time) * U256::from(50) / time_limit;
            bonus_ratio
        } else {
            U256::ZERO
        };

        let base_points = if is_correct {
            U256::from(100)
        } else {
            U256::ZERO
        };

        let difficulty_multiplier = if difficulty == U8::ZERO {
            U256::from(100) // Easy: 1x
        } else if difficulty == U8::from(1) {
            U256::from(150) // Medium: 1.5x
        } else if difficulty == U8::from(2) {
            U256::from(200) // Hard: 2x
        } else {
            U256::from(100)
        };

        let mut points = (base_points + time_bonus) * difficulty_multiplier / U256::from(100);

        // Calculate new streak
        let new_streak = if is_correct {
            player_current_streak + U256::from(1)
        } else {
            U256::ZERO
        };

        // Add streak bonus
        if is_correct && new_streak >= U256::from(2) {
            let streak_bonus = new_streak * U256::from(10);
            points = points + streak_bonus;
        }

        // Now do all mutations
        let mut session_mut = self.sessions.setter(session_id);
        let mut player_mut = session_mut.players.setter(player_address);

        if is_correct {
            player_mut.current_streak.set(new_streak);

            if new_streak > player_best_streak {
                player_mut.best_streak.set(new_streak);
            }

            player_mut.correct_answers.set(player_correct_answers + U256::from(1));
        } else {
            player_mut.current_streak.set(U256::ZERO);
        }

        player_mut.score.set(player_score + points);
        player_mut.total_response_time.set(player_total_response_time + response_time);

        // Record answer - need to handle the temporary value
        let mut answers_setter = session_mut.answers.setter(question_index);
        let mut answer = answers_setter.setter(player_address);
        answer.answer_hash.set(answer_hash);
        answer.submit_time.set(current_time);
        answer.is_correct.set(is_correct);
        answer.points_earned.set(points);

        Ok(points)
    }

    pub fn end_session(&mut self, session_id: U256) -> Result<Address, TriviaChainError> {
        let session_timestamp = self.vm().block_timestamp();

        // Read all session data first
        let session = self.sessions.getter(session_id);
        let host_check = session.host.get();
        let status_check = session.status.get();
        let player_count = session.player_list.len();

        // Check authorization
        if host_check != self.vm().msg_sender() {
            return Err(TriviaChainError::Unauthorized(Unauthorized {}));
        }

        if status_check != U8::from(1) {
            return Err(TriviaChainError::SessionNotActive(SessionNotActive {}));
        }

        // Find winner and collect all player data
        let mut highest_score = U256::ZERO;
        let mut winner_address = Address::ZERO;
        let mut player_data = Vec::new();

        for i in 0..player_count {
            let player_address = session.player_list.get(i).unwrap();
            let player = session.players.getter(player_address);
            let player_score = player.score.get();
            let player_correct = player.correct_answers.get();
            let player_best_streak = player.best_streak.get();

            player_data.push((player_address, player_score, player_correct, player_best_streak));

            if player_score > highest_score {
                highest_score = player_score;
                winner_address = player_address;
            }
        }

        // Update session status
        let mut session_mut = self.sessions.setter(session_id);
        session_mut.status.set(U8::from(2)); // Completed
        session_mut.end_time.set(U256::from(session_timestamp));
        session_mut.winner.set(winner_address);

        // Update winner stats
        if winner_address != Address::ZERO {
            let winner_total_wins = self.player_stats.getter(winner_address).total_wins.get();
            let mut winner_stats = self.player_stats.setter(winner_address);
            winner_stats.total_wins.set(winner_total_wins + U256::from(1));
        }

        // Update all player stats
        for (player_address, player_score, player_correct, player_best_streak) in player_data {
            // Read existing stats
            let stats_getter = self.player_stats.getter(player_address);
            let games_played = stats_getter.games_played.get();
            let total_score = stats_getter.total_score.get();
            let best_score = stats_getter.best_score.get();
            let total_correct = stats_getter.total_correct_answers.get();
            let longest_streak = stats_getter.longest_streak.get();

            // Update stats
            let mut stats = self.player_stats.setter(player_address);
            stats.games_played.set(games_played + U256::from(1));
            stats.total_score.set(total_score + player_score);

            if player_score > best_score {
                stats.best_score.set(player_score);
            }

            stats.total_correct_answers.set(total_correct + player_correct);

            if player_best_streak > longest_streak {
                stats.longest_streak.set(player_best_streak);
            }
        }

        // Update active sessions count
        let active_count = self.active_sessions_count.get();
        self.active_sessions_count.set(active_count - U256::from(1));

        Ok(winner_address)
    }

    pub fn get_session_info(&self, session_id: U256) -> (Address, U8, U256, U256, Address) {
        let session = self.sessions.getter(session_id);
        (
            session.host.get(),
            session.status.get(),
            session.player_count.get(),
            session.current_question_index.get(),
            session.winner.get(),
        )
    }

    pub fn get_player_score(&self, session_id: U256, player: Address) -> U256 {
        self.sessions.getter(session_id).players.getter(player).score.get()
    }

    pub fn get_player_stats(&self, player: Address) -> (U256, U256, U256, U256) {
        let stats = self.player_stats.getter(player);
        (
            stats.games_played.get(),
            stats.total_wins.get(),
            stats.best_score.get(),
            stats.longest_streak.get(),
        )
    }

    pub fn get_leaderboard(&self, session_id: U256) -> Vec<(Address, U256)> {
        let session = self.sessions.getter(session_id);
        let mut leaderboard = Vec::new();

        for i in 0..session.player_list.len() {
            let player_address = session.player_list.get(i).unwrap();
            let player = session.players.getter(player_address);
            leaderboard.push((player_address, player.score.get()));
        }

        leaderboard.sort_by(|a, b| b.1.cmp(&a.1));
        leaderboard
    }
}

