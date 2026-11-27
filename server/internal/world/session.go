package world

import (
	"context"
	"sync"
	"time"

	"google.golang.org/protobuf/proto"

	pb "github.com/tekne-studio/planet/server/internal/protocol"
)

// outboxDepth is how many frames a client may fall behind before it is
// dropped as too slow to follow: a second of ticks.
const outboxDepth = 32

// session is one connection inside a world. The actor owns its fields; the
// reader and writer goroutines touch only the channels.
type session struct {
	id       uint32
	identity Identity
	avatar   string
	// What the person is called here: the account's name, or the one a
	// visitor gave. Changed by Rename.
	name   string
	stance *pb.Stance
	// When the session sounded its last voices, and asked its last days.
	sounded window
	dayed   window
	// What the session may relay at once, refilled at RelayPerSecond up to
	// RelayBurst, and when it was last refilled.
	relayable float64
	refilled  time.Time
	conn      Conn
	// Frames for the writer. Closed by the actor when the session ends.
	out chan []byte
	// Closed once, when the actor lets the session go.
	ended chan struct{}
	once  sync.Once
}

func newSession(identity Identity, avatar, name string, conn Conn) *session {
	if identity.Name != "" {
		name = identity.Name
	}
	return &session{
		identity:  identity,
		avatar:    avatar,
		name:      name,
		conn:      conn,
		out:       make(chan []byte, outboxDepth),
		ended:     make(chan struct{}),
		relayable: RelayBurst,
	}
}

// window is when a session did one thing lately, oldest first: at most a
// burst of times.
type window []time.Time

// allows says whether one more at `now` is taken: past `burst` within
// `span` it is not, and is not counted.
func (w *window) allows(now time.Time, burst int, span time.Duration) bool {
	if len(*w) == burst {
		if now.Sub((*w)[0]) < span {
			return false
		}
		*w = (*w)[1:]
	}
	*w = append(*w, now)
	return true
}

// sounds says whether a voice sounded at `now` is heard: one more past
// SoundBurst within SoundWindow is not, and is not counted.
func (s *session) sounds(now time.Time) bool {
	return s.sounded.allows(now, SoundBurst, SoundWindow)
}

// asksDay says whether an ask for a day at `now` is taken: one more past
// DayBurst within DayWindow is not, and is not counted.
func (s *session) asksDay(now time.Time) bool {
	return s.dayed.allows(now, DayBurst, DayWindow)
}

// relays says whether a payload relayed at `now` passes: one past the
// session's rate is not, and is not counted.
func (s *session) relays(now time.Time) bool {
	if !s.refilled.IsZero() {
		s.relayable = min(RelayBurst, s.relayable+now.Sub(s.refilled).Seconds()*RelayPerSecond)
	}
	s.refilled = now
	if s.relayable < 1 {
		return false
	}
	s.relayable--
	return true
}

func (s *session) peer() *pb.Peer {
	return &pb.Peer{
		Session: s.id,
		Name:    s.name,
		Visitor: s.identity.Visitor(),
		Avatar:  s.avatar,
		Stance:  s.stance,
	}
}

// who is the session as a plugin sees it.
func (s *session) who() Who {
	return Who{Session: s.id, Identity: s.identity, Name: s.name, Stance: s.stance}
}

// send queues a message for the writer, dropping the session if it is behind.
func (s *session) send(message *pb.ServerMessage) {
	if frame, err := proto.Marshal(message); err == nil {
		s.offer(frame)
	}
}

// offer queues a frame without waiting. False when the client is behind.
func (s *session) offer(frame []byte) bool {
	select {
	case <-s.ended:
		return false
	default:
	}
	select {
	case s.out <- frame:
		return true
	default:
		return false
	}
}

// end is the actor letting go: the writer drains what is queued and the
// reader stops feeding the inbox.
func (s *session) end() {
	s.once.Do(func() {
		close(s.ended)
		close(s.out)
	})
}

// run pumps the connection until it ends: frames in to the actor's inbox,
// frames out from the outbox. Returns when the actor has let the session go.
func (s *session) run(ctx context.Context, a *actor) error {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	go func() {
		for frame := range s.out {
			if err := s.conn.Write(ctx, frame); err != nil {
				cancel()
				return
			}
		}
	}()

	var failure error
	for {
		frame, err := s.read(ctx)
		if err != nil {
			failure = err
			break
		}
		var message pb.ClientMessage
		if err := proto.Unmarshal(frame, &message); err != nil {
			failure = err
			break
		}
		select {
		case a.inbox <- inbound{kind: messageKind, session: s, message: &message}:
		case <-s.ended:
		}
		if s.gone() {
			break
		}
	}

	// Either the link died or the actor dropped us. Say goodbye in both
	// cases; the actor ignores a leave for a session it no longer holds.
	select {
	case a.inbox <- inbound{kind: leaveKind, session: s}:
	case <-s.ended:
	}
	<-s.ended
	cancel()
	return failure
}

func (s *session) read(ctx context.Context) ([]byte, error) {
	ctx, cancel := context.WithTimeout(ctx, readTimeout)
	defer cancel()
	return s.conn.Read(ctx)
}

func (s *session) gone() bool {
	select {
	case <-s.ended:
		return true
	default:
		return false
	}
}
