h08537
s 00000/00001/00881
d D 1.6 01/10/23 00:05:27 bmc 7 6
c 1000017 Ernie needs levels other than "Hard" and "Impossible"
e
s 00010/00023/00872
d D 1.5 01/10/22 22:30:58 ahl 6 5
c 1000013 props for airslides (I suck)
e
s 00035/00208/00860
d D 1.4 01/10/22 21:11:05 ahl 5 4
c 1000013 props for airslides (lint)
e
s 00126/00098/00942
d D 1.3 01/10/22 17:56:46 ahl 4 3
c 1000013 props for airslides
e
s 00000/00000/01040
d D 1.2 01/10/21 19:25:08 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:27 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTGame.C
c Name history : 1 0 src/game/BTGame.C
e
s 01040/00000/00000
d D 1.1 01/10/20 13:35:26 bmc 1 0
c date and time created 01/10/20 13:35:26 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTGame.C                                            */
/*    ASSN:                                                     */
/*    DATE: Mon Feb 21 19:43:28 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <assert.h>

#include "BattleTris.H"

#include "BTBazaar.H"
#include "BTPimp.H"
#include "BTPieceManager.H"
#include "BTScoreManager.H"
#include "BTSoundManager.H"
#include "BTCommManager.H"
#include "BTWeaponManager.H"
#include "BTRecon.H"
#include "BTGameStats.H"
#include "BTComputer.H"
#include "BTGame.H"
#include "BTChallengeDialog.H"
#include "BTPixmap.H"
#include "BTDisplay.H"
#include "BTBox.H"

#define BT_BOARD_X 30
#define BT_BOARD_Y 30
#define BT_MESSAGE_X 305
#define BT_MESSAGE_Y 625

KeySym ks;
XComposeStatus cs;
char b[10];
static int initialized_ = 0;

// Global variable used for translation callbacks
BTGame *GAME = NULL;

BTGame::BTGame (BTWidget *parent, BTSoundManager *sound_manager, 
		BTCommManager *comm_manager,
		BTPimp *pimp, BTBazaar *bazaar, BTPixmap *gimp ) 
: BTRingNode(), pimp_ (pimp), bazaar_ (bazaar), sound_manager_ (sound_manager),
  comm_manager_(comm_manager), gimp_pixmap_(gimp),
  piece_manager_(0), board_(0), current_piece_(0)
{
  GAME = this;
  
  sliding_ = 0;
I 4
D 6
  airslide_ = 0;
E 6
I 5
  drop_ = 0;
E 5
E 4
  Arg args[20];
  int n = 0;
  
  initial_board_ = board_ = 0;
  
  stats_ = new BTGameStats();
  chal_dialog_ = 0;
  slick_dir_ = 0;
  paused_ = 0;
  done_baz_ = 0;
  op_done_baz_ = 0;
  in_baz_ = 0;
  started_ = 0;
  swapper_ = 0;
  computer_ = 0;
  condor_on_ = 0;
  
  parent_ = parent;
  
  form_ = new BTFormWidget (parent, "BTGame", BT_GAME_WIDTH, BT_GAME_HEIGHT, 
			    BT_GAME_FRAC_BASE);
  
  drawing_area_ = new BTDrawingAreaWidget (form_, "drawing_area", 0, 
					   BT_BOX_WTH * BT_BOARD_WTH,
					   BT_BOX_HGT * BT_BOARD_HGT,
					   30, 30);
  
  drawing_area_->attachLeftNone();
  drawing_area_->attachTopNone();
  drawing_area_->attachRightNone();
  drawing_area_->attachBottomNone();
  
  drawing_area_->size( BT_BOARD_X, BT_BOARD_Y );
  
  XtVaSetValues (*drawing_area_, XtVaTypedArg, XmNbackground, XmRString, "black", 6, 0);
  
  message_ = new BTLabelWidget (form_, "message", " ");
  
  message_->attachLeftNone();
  message_->attachTopNone();
  message_->attachRightNone();
  message_->attachBottomNone();
  
  message_->size( BT_MESSAGE_X, BT_MESSAGE_Y, BT_BOX_WTH * BT_BOARD_WTH - 20, 100 );
  
  drawing_area_->manage();
  message_->manage();
  
  drawing_area_->addExposeCallback( _gamecb2_, this );
  drawing_area_->setKbdCallback(_keypress2_, this);
  
  XtOverrideTranslations( *drawing_area_, g_resources.keymappings );
  
  bazaar_->done_button_->addActivateCallback(_bazcb2_, this);
  
  recon_ = new BTRecon (parent_, form_, gimp_pixmap_);
  weapon_manager_ = new BTWeaponManager (form_, BT_GAME_ARSENAL_X,
					 BT_GAME_ARSENAL_Y, comm_manager_);
  
  score_manager_ = new BTScoreManager (weapon_manager_,recon_, form_,
				       BT_GAME_SCORE_X, 
				       BT_GAME_SCORE_Y);
  
  // Establish token ring
  comm_manager_ = comm_manager;
  next_ = comm_manager_;
  comm_manager_->next (this);
  
  initialized_ = 0;
  
  // Initialize all timeouts.
  timeout_[BT_DROP_TIMEOUT].callback_ = timeout_CB;
  timeout_[BT_SLIDE_TIMEOUT].callback_ = slidetime_CB;
  
  timeout_[BT_HATTER_TIMEOUT].callback_ = hattertime_CB;
  timeout_[BT_HATTER_TIMEOUT].time_ = 20;
  
  timeout_[BT_SLICK_TIMEOUT].callback_ = slicktime_CB;
  timeout_[BT_SLICK_TIMEOUT].time_ = 20;
  
  timeout_[BT_JEOPARDY_TIMEOUT].callback_ = jeotime_CB;
  timeout_[BT_JEOPARDY_TIMEOUT].time_ = 7600;
  
  base_drop_time_ = BT_DROP_TIME;
  fast_drop_time_ = BT_FAST_DROP_TIME;
  
  drop_time_ = &timeout_[BT_DROP_TIMEOUT].time_;
  slide_time_ = &timeout_[BT_SLIDE_TIMEOUT].time_;
  
  *drop_time_ = BT_DROP_TIME;
  *slide_time_ = BT_SLIDE_TIME;

}

BTGame::~BTGame() {
  if(initialized_) {
    if (current_piece_)
      current_piece_->reset();
    delete board_;
    delete piece_manager_;
  }
  delete score_manager_;
  delete weapon_manager_;
  delete recon_;
  delete message_;
  delete drawing_area_;
  delete form_;
  delete stats_;
}

void BTGame::_keypress2_(char key, void *data) {
  BTGame *t = (BTGame *) data;
  if (t->in_baz_ == 1) 
    return;
  if (!initialized_) {
    BTDebug ("Keypress received before game was initialized.  Ignoring.");
    return;
  }
  t->keyPressed(key);
}

void BTGame::_pausekey2_ (char key, void *data) {
  BTGame *t = (BTGame *) data;
  if (t->in_baz_ == 1) 
    return;
  if (key == 'U' || key == 'u' || key == 'P' || key == 'p')
    t->sendPlusMe (BT_PAUSE, 0);
}

void BTGame::_bazcb2_ (BTWidget *w, void *thisp) {
  BTGame *t = (BTGame *) thisp;
  if (t->done_baz_ == 1) 
    return;
  t->done_baz_ = 1;
  t->send (BT_END_BAZ, 0);
  if (!t->op_done_baz_) {
    if(g_resources.r_rated == False)
      t->bazaar_->setMessage("Waiting for opponent...");
    else 
      t->bazaar_->setMessage("Waiting for fat slut...");
    t->bazaar_->message_->manage();
    t->bazaar_->dimButtons();
  }
  else
    t->leaveBazaar();
}

void BTGame::addTimeOut (int id) {
  
  // If the game is paused, then this timeout should be created 
  // in a paused state.
  
  // Is Bryan a gimp?
  removeTimeOut(id);
  
  if (!timeout_[id].rep_ && paused_) {
    timeout_[id].active_ = 1;
    timeout_[id].paused_ = 1;
    return;
  }
  
  if (!timeout_[id].rep_) {
    assert (!timeout_[id].paused_);
    
    timeout_[id].active_ = 1;
    timeout_[id].rep_ = DISPLAY->addTimeout(timeout_[id].time_, 
					    timeout_[id].callback_, this);
    return;
  }
  assert (timeout_[id].active_ && !timeout_[id].paused_);
}

void BTGame::removeTimeOut (int id) {
  if (timeout_[id].active_) {
    timeout_[id].paused_ = 0;
    if (timeout_[id].rep_)
      DISPLAY->removeTimeout(timeout_[id].rep_);
    timeout_[id].rep_ = 0;
    timeout_[id].active_ = 0;
  }
}

void BTGame::expireTimeOut (int id) {
  // Must be called at the beginning of _every_ timeout
  timeout_[id].rep_ = 0;
}

void BTGame::clearTimeOut (int id) {
  if (!timeout_[id].paused_) 
    timeout_[id].active_ = 0;
}

void BTGame::pauseTimeOut (int id) {
  if (timeout_[id].active_) {
    assert (!timeout_[id].paused_);
    
    if (timeout_[id].rep_)
      DISPLAY->removeTimeout(timeout_[id].rep_);
    timeout_[id].rep_ = 0;
    
    timeout_[id].paused_ = 1;
  }
}

void BTGame::unpauseTimeOut (int id) {
  if (timeout_[id].paused_) {
    assert (timeout_[id].active_ && !timeout_[id].rep_);
    timeout_[id].paused_ = 0;
    timeout_[id].rep_ = DISPLAY->addTimeout(timeout_[id].time_,
					    timeout_[id].callback_, this);
  } else if (timeout_[id].active_) {
  }
}

void BTGame::pauseAllTimeOuts() {
  for (int i = 0; i < BT_MAX_GAME_TIMEOUT; i++)
    pauseTimeOut (i);
}

void BTGame::unpauseAllTimeOuts() {
  for (int i = 0; i < BT_MAX_GAME_TIMEOUT; i++)
    unpauseTimeOut (i);
}

void BTGame::removeAllTimeOuts() {
  for (int i = 0; i < BT_MAX_GAME_TIMEOUT; i++)
    removeTimeOut (i);
}

void BTGame::timeout (unsigned long *) {
  expireTimeOut (BT_DROP_TIMEOUT);
  
  if (in_baz_ == 1) {
    addTimeOut (BT_DROP_TIMEOUT);
    pauseTimeOut (BT_DROP_TIMEOUT);
    return;
  }
  
  addTimeOut (BT_DROP_TIMEOUT);
  drop();
}

void BTGame::slidetime (unsigned long *) {
  
  expireTimeOut (BT_SLIDE_TIMEOUT);
  if (in_baz_ == 1) {
    addTimeOut (BT_SLIDE_TIMEOUT);
    pauseTimeOut (BT_SLIDE_TIMEOUT);
    return;
  }
I 4

E 4
  place();
  sliding_ = 0;
D 7
  runComputer();
E 7
  clearTimeOut (BT_SLIDE_TIMEOUT);
}

void BTGame::hattertime (unsigned long *) {
  expireTimeOut (BT_HATTER_TIMEOUT);
  
  if (in_baz_ == 1) {
    addTimeOut (BT_HATTER_TIMEOUT);
    pauseTimeOut (BT_HATTER_TIMEOUT);
    return;
  }
  
  addTimeOut (BT_HATTER_TIMEOUT);
  current_piece_->rotate();
  DISPLAY->flush();
}

void BTGame::slicktime (unsigned long *) {
D 4
  expireTimeOut (BT_SLICK_TIMEOUT);
E 4
I 4
D 5
	expireTimeOut (BT_SLICK_TIMEOUT);
E 5
I 5
	expireTimeOut(BT_SLICK_TIMEOUT);
E 5
E 4
  
D 4
  if (in_baz_ == 1) {
    addTimeOut (BT_SLICK_TIMEOUT);
    pauseTimeOut (BT_SLICK_TIMEOUT);
    return;
  }
  if (slick_dir_ == 0) {
    if (!(current_piece_->moveTo(left_x_ + x_, y_)))
      slick_dir_ = 1;
    else
      x_ += left_x_;
  } else {
    if (!(current_piece_->moveTo(right_x_ + x_, y_)))
      slick_dir_ = 0;
    else
      x_ += right_x_;
  }
  addTimeOut (BT_SLICK_TIMEOUT);
  DISPLAY->flush();
E 4
I 4
	if (in_baz_ == 1) {
D 5
		addTimeOut (BT_SLICK_TIMEOUT);
		pauseTimeOut (BT_SLICK_TIMEOUT);
E 5
I 5
		addTimeOut(BT_SLICK_TIMEOUT);
		pauseTimeOut(BT_SLICK_TIMEOUT);
E 5
		return;
	}
	if (slick_dir_ == 0) {
D 5
		if (!(current_piece_->moveTo(left_x_ + x_, y_)))
E 5
I 5
D 6
		if (!(current_piece_->moveTo(left_x_ + x_, y_))) {
E 6
I 6
		if (!(current_piece_->moveTo(left_x_ + x_, y_)))
E 6
E 5
			slick_dir_ = 1;
D 5
		else
E 5
I 5
D 6
		} else {
E 6
I 6
		else
E 6
E 5
			x_ += left_x_;
I 5
D 6
			airslide_ = 0;
		}
E 6
E 5
	} else {
D 5
		if (!(current_piece_->moveTo(right_x_ + x_, y_)))
E 5
I 5
D 6
		if (!(current_piece_->moveTo(right_x_ + x_, y_))) {
E 6
I 6
		if (!(current_piece_->moveTo(right_x_ + x_, y_)))
E 6
E 5
			slick_dir_ = 0;
D 5
		else
E 5
I 5
D 6
		} else {
E 6
I 6
		else
E 6
E 5
			x_ += right_x_;
I 5
D 6
			airslide_ = 0;
		}
E 6
E 5
	}

D 5
	airslide_ = 0;

	addTimeOut (BT_SLICK_TIMEOUT);
E 5
I 5
	addTimeOut(BT_SLICK_TIMEOUT);
E 5
	DISPLAY->flush();
E 4
}

void BTGame::jeotime (unsigned long *) {
  expireTimeOut (BT_JEOPARDY_TIMEOUT);
  
  if (in_baz_ == 1) {
    sound_manager_->playJeopardy();
    addTimeOut (BT_JEOPARDY_TIMEOUT);
  } else {
    clearTimeOut (BT_JEOPARDY_TIMEOUT);
  }
}

void BTGame::pause(int no_send) {
  if (!paused_) {
    message_->setLabel("Paused...");
    
    pauseAllTimeOuts();
    paused_ = 1;
    if ( ! no_send )
      send(BT_PAUSE, 0);
/*
  drawing_area_->setKbdCallback( _pausekey2_, this );
  //    XtRemoveEventHandler (rep_, KeyPressMask, FALSE, _keypress_, this);
  //    XtAddEventHandler (rep_, KeyPressMask, FALSE, _pausekey_, this);
  */
  }
  else {
/*
  drawing_area_->setKbdCallback( _keypress2_, this );
  //    XtRemoveEventHandler (rep_, KeyPressMask, FALSE, _pausekey_, this);
  //    XtAddEventHandler (rep_, KeyPressMask, FALSE, _keypress_, this);
  */
    if (!no_send)
      send(BT_PAUSE, 0);
    message_->setLabel(opponent_msg_);
    unpauseAllTimeOuts();
    paused_ = 0;
  }
}

void BTGame::unpause() {
}

void BTGame::exposeEvent () {
  if (!initialized_) {
    initialized_ = 1;
    started_ = 0;
    board_  = new BTBoardManager (weapon_manager_);
    initial_board_ = board_;
    piece_manager_ = new BTPieceManager (drawing_area_, board_,
					 gimp_pixmap_);
    
    next_ = board_;
    board_->next (piece_manager_);
    piece_manager_->next (sound_manager_);
    sound_manager_->next (score_manager_);
    score_manager_->next (comm_manager_);
    comm_manager_->next (weapon_manager_);
    weapon_manager_->next (recon_);
    recon_->next (this);
    
    board_->redraw();
  } 
  if (!started_) {
    reset();
    if (computer_)
      condor();
    board_->redraw();
    started_ = 1;
    startGame();
  } else
    board_->redraw();
}

void BTGame::leaveBazaar() {
  
  BTDebug ("Everyone has exited...now leaving bazaar.");
  parent_->size( -1, -1, old_width_, old_height_ );
  bazaar_->hide();
  stopwatch_.start();
  done_baz_ = 0;
  op_done_baz_ = 0;
  in_baz_ = 0;
  weapon_manager_->update();
  
  removeTimeOut (BT_JEOPARDY_TIMEOUT);
  unpauseAllTimeOuts();
  current_piece_->redraw();
  board_->redraw();
  
  // Need to put key focus on the game screen
  drawing_area_->focus();
  
}

void BTGame::runComputer()
{
  if ( computer_ &&  ( in_baz_ != 1 ) && !paused_ )
    computer_->run();
}

void BTGame::receive (BTRingPacket *packet) {
  if (initial_board_ && (board_ != initial_board_))
    abort();
  switch (packet->token) {
  case BT_START: {
    char c[255];
    if (packet->data != 0)
      strcpy (c, (char *) packet->data);
    BTDebug ("Your opponent is " << c);
    strcpy (opponent_msg_, "Playing ");
    strcat (opponent_msg_, c);
    
    if (!computer_)
      sound_manager_->start();
    message_->setLabel (opponent_msg_);
    
    // After this stuff happens, the exposeEvent will take care of
    // the rest of starting up...

    form_->manage();

    break;
  }
  case BT_BOARD: {
    if ( in_baz_ == 1) {
      pass(packet);
      return;
    }
    BTBoard *board = (BTBoard *) packet->data;
    BTDebug ("Board received token, motivation is " << board->motivation());
    if (board->motivation() == BT_SWAP) {
      if (!swapper_) {
	if (weapon_manager_->BTActive[BT_BOTTLE]) {
          weapon_manager_->remaining_[BT_BOTTLE] = 0;
          send (BT_WPN_OFF, (*pimp_)[BT_BOTTLE]);
	}
	if (weapon_manager_->BTActive[BT_UPBYSIDE]) {
          weapon_manager_->remaining_[BT_UPBYSIDE] = 0;
          sendPlusMe (BT_WPN_OFF, (*pimp_)[BT_UPBYSIDE]);
	}
      }
      BTBoard temp (board_);
      board_->newBoard (board);
      BTDebug ("Swapped in new board.");
      if (!swapper_) {      
        BTDebug ("I did not initiate the swap -- must respond.");
        temp.motivation(BT_SWAP);
        comm_manager_->sendBoard (&temp);
      }
      swapper_ = 0;
    }
    break;
  }
  case BT_WPN_LAUNCH: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_SWAP: {
      swapper_ = 1;
      if (weapon_manager_->BTActive[BT_BOTTLE]) {
        weapon_manager_->remaining_[BT_BOTTLE] = 0;
        weapon_manager_->BTActive[BT_BOTTLE] = 0;
        send (BT_WPN_OFF, (*pimp_)[BT_BOTTLE]);
      }
      if (weapon_manager_->BTActive[BT_UPBYSIDE]) {
        weapon_manager_->remaining_[BT_UPBYSIDE] = 0;
        weapon_manager_->BTActive[BT_UPBYSIDE] = 0;
        sendPlusMe (BT_WPN_OFF, (*pimp_)[BT_UPBYSIDE]);
      }
      BTBoard temp (board_);
      temp.motivation(BT_SWAP);
      comm_manager_->sendBoard (&temp);
      break;
    }
    default:
      break;
    }
    break;
  }
  case BT_PAUSE: {
    pause(1);
    break;
  }
  case BT_WPN_ON: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_UPBYSIDE: {
      def_y_ = BT_BOARD_HGT - 4;  
      delta_y_ = -1;
      left_x_ = 1;
      right_x_ = -1;
      break;
    }
    case BT_HATTER: {
      addTimeOut (BT_HATTER_TIMEOUT);
      break;
    }     
    case BT_SLICK: {
      addTimeOut (BT_SLICK_TIMEOUT);
      break;
    }
      
    case BT_SPEEDY: {
      base_drop_time_ >>= 1;
      break;
    }
    case BT_MEADOW: {
      fast_drop_time_ <<= 1;
      base_drop_time_ <<= 1;
      break;
    }
    }
    break;
  }
  case BT_START_BAZ: {
    
    old_width_ = parent_->width();
    old_height_ = parent_->height();
    
    parent_->size( -1, -1, BT_BAZAAR_WIDTH, BT_BAZAAR_HEIGHT);
    
    pauseAllTimeOuts();
    in_baz_ = 1;
    done_baz_ = 0;
    
    BTDebug ("Received start baz token!");
    
    stopwatch_.stop();
    bazaar_->show (score_manager_->rep_.funds_, 
		   weapon_manager_->arsenal_, weapon_manager_->BTActive[BT_CARTER]); 
    break;
  }
  case BT_END_BAZ: {
    BTDebug ("Your opponent has left the bazaar...");
    op_done_baz_ = 1;
    if (done_baz_) 
      leaveBazaar();
    else {
      addTimeOut (BT_JEOPARDY_TIMEOUT);
      if(g_resources.r_rated == False)
	bazaar_->setMessage("Your opponent is waiting...");
      else
	bazaar_->setMessage("Fuckface is getting angsty.");
      bazaar_->message_->manage();
    }
    break;
  }
    
  case BT_ERR: {
    cleanUp();
    break;
  }
    
  case BT_GAME_OVER: {
    cleanUp();
    if(g_resources.r_rated == False)
      message_->setLabel ("You suck!");
    else
      message_->setLabel ("Nice loss, shithead.");
    break;
  }
    
  case BT_DEAD: {
    cleanUp();
    if(g_resources.r_rated == False)
      message_->setLabel ("Yer huge!");
    else
      message_->setLabel ("Yer the shit!");
    break;
  }
    
  case BT_WPN_OFF: {
    BTWeapon *wpn = (BTWeapon *) packet->data; 
    switch (wpn->token()) {
    case BT_UPBYSIDE: {
      def_x_ = BT_DEFAULT_X;
      def_y_ = BT_DEFAULT_Y;
      delta_y_ = 1;
      left_x_ = -1;
      right_x_ = 1;
      break;
    }
    case BT_HATTER: {
      removeTimeOut (BT_HATTER_TIMEOUT);
      break;
    }
    case BT_SLICK: {
      removeTimeOut (BT_SLICK_TIMEOUT);
      break;
    }
      
    case BT_SPEEDY: {
      base_drop_time_ <<= 1;
      break;
    }      
    case BT_MEADOW: {
      base_drop_time_ >>= 1;
      fast_drop_time_ >>= 1;
      break;
    }
    }
    break;
  }
  }
  pass (packet);
}

void BTGame::moveLeft() {
D 4
  if (paused_) return;
  if (current_piece_ && current_piece_->moveTo (x_+left_x_, y_)) { 
    if (sliding_)
      sliding_++;
    x_ += left_x_; 
    DISPLAY->flush();
  }
E 4
I 4
	if (paused_)
		return;
	if (current_piece_ && current_piece_->moveTo (x_+left_x_, y_)) { 
D 6
		if (sliding_) {
E 6
I 6
		if (sliding_)
E 6
			sliding_++;
D 6
			airslide_ = 0;
		} else {
			airslide_ = 1;
		}
E 6
		x_ += left_x_; 
		DISPLAY->flush();
	}
E 4
}

void BTGame::moveRight() {
D 4
  if (paused_) return;
  if (current_piece_ && current_piece_->moveTo (x_+right_x_, y_)) {
    if (sliding_)
      sliding_++;
    x_ += right_x_;
    DISPLAY->flush();
  }
E 4
I 4
	if (paused_)
		return;
	if (current_piece_ && current_piece_->moveTo (x_+right_x_, y_)) {
D 6
		if (sliding_) {
E 6
I 6
		if (sliding_)
E 6
			sliding_++;
D 6
			airslide_ = 0;
		} else {
			airslide_ = 1;
		}
E 6
		x_ += right_x_;
		DISPLAY->flush();
	}
E 4
}

void BTGame::rotate() {
  if (paused_) return;
  if (current_piece_) {
    current_piece_->rotate ();
    DISPLAY->flush();
  }
}

void BTGame::condor() {
  if (computer_) {
    if (condor_on_) {
      sendPlusMe (BT_CONDOR_OFF, 0);
    } else {
      send (BT_WPN_LAUNCH, (*pimp_)[BT_CONDOR]);
      recon_->spy_on_ = 65535;
    }    
    condor_on_ ^= 1;
  }
}

void BTGame::beginDrop() {
D 5
  if (paused_) return;
  if ( *drop_time_ == fast_drop_time_ )
    return;
  removeTimeOut (BT_DROP_TIMEOUT);
  removeTimeOut (BT_SLICK_TIMEOUT);
E 5
I 5
	if (paused_)
		return;

	drop_ = 1;

	if (*drop_time_ == fast_drop_time_)
		return;

	removeTimeOut(BT_DROP_TIMEOUT);
	removeTimeOut(BT_SLICK_TIMEOUT);
E 5
  
D 5
  *drop_time_ = fast_drop_time_;
  score_manager_->rep_.score_ += BT_BOARD_HGT- y_;
E 5
I 5
	*drop_time_ = fast_drop_time_;
	score_manager_->rep_.score_ += BT_BOARD_HGT- y_;
E 5
  
D 5
  addTimeOut (BT_DROP_TIMEOUT);
E 5
I 5
	addTimeOut(BT_DROP_TIMEOUT);
E 5
}

void BTGame::keyPressed (char c) {
D 5
/*
  if ((c == 'm') && !(m >> 1) && !(m % 2)) m |= 2;
  else if ((c == ';') && ((m % 4) >= 2)) m -= 1;
  else if ((c == 'm') && (m << 1 == 2)) {
    short funds = 200;
    send (BT_FUNDS, &funds);
  } else m = 0;
*/  
  if (c >= '0' && c <= '9') {
    weapon_manager_->launchWeapon ((int) (c - '0'));
    return;
  }
  
  switch (c) { 
/* This is taken care of in the translations
   
   case 'j': case 'J': {
   if (current_piece_ && current_piece_->moveTo (x_+left_x_, y_)) { 
D 4
   if (sliding_)
   sliding_++;
   x_ += left_x_; 
   DISPLAY->flush();
E 4
I 4
     if (sliding_)
       sliding_++;
     x_ += left_x_; 
     DISPLAY->flush();
E 4
   }
   break;
   }
   case 'k': case 'K': {
   if (current_piece_) {
   current_piece_->rotate ();
   DISPLAY->flush();
   }
   break;
   }
   case 'l': case 'L': {
   if (current_piece_ && current_piece_->moveTo (x_+right_x_, y_)) {
D 4
   if (sliding_)
   sliding_++;
   x_ += right_x_;
   DISPLAY->flush();
E 4
I 4
     if (sliding_)
       sliding_++;
     x_ += right_x_;
     DISPLAY->flush();
E 4
   }
   break;
   }
   case 'p': case 'P': {
   pause();
   break;
   }
   case 'c': case 'C': {    
   if (computer_) {
   if (condor_on_) {
   recon_->spy_on_ = 0;
   send (BT_CONDOR_OFF, 0);
   } else {
   send (BT_WPN_LAUNCH, (*pimp_)[BT_CONDOR]);
   recon_->spy_on_ = 65535;
   }    
   condor_on_ = 1 - condor_on_;
   }
   break;
   }
   case ' ': {
   if ( (!current_piece_) || (*drop_time_ == fast_drop_time_ ) )
   break;
   removeTimeOut (BT_DROP_TIMEOUT);
   removeTimeOut (BT_SLICK_TIMEOUT);
   
   *drop_time_ = fast_drop_time_;
   score_manager_->rep_.score_ += BT_BOARD_HGT- y_;
   
   addTimeOut (BT_DROP_TIMEOUT);
   break;
   }  
   */
    
/*
  case 's': {
  sendPlusMe (BT_WPN_LAUNCH, (*pimp_)[BT_KEATING]);
  break;
  }
  
  */  
/*
  case 's': {
  BTWeapon temp (BT_SUSAN); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'f': {
  BTWeapon temp (BT_FALL_OUT); 
  sendPlusMe (BT_WPN_ON, &temp);
  break;
  }
  case 'F': {
  BTWeapon temp2 (BT_FORCE); 
  sendPlusMe (BT_WPN_ON, &temp2);
  break;
  }
  
  case 'F': {
  BTWeapon temp (BT_FALL_OUT); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'H': {
  BTWeapon temp (BT_HATTER); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'b': {
  BTWeapon temp (BT_BLIND); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'B': {
  BTWeapon temp (BT_BOTTLE); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'E': {
  sendPlusMe (BT_WPN_LAUNCH, (*pimp_)[BT_REAGAN]);
  break;
  }
  case 's':  {
  BTWeapon temp (BT_SLICK);
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  
  case 'h': {
  BTWeapon temp (BT_HATTER);
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'D': {
  BTWeapon temp (BT_LAWYERS); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'h': {
  BTWeapon temp (BT_NICE_DAY); 
  sendPlusMe (BT_WPN_ON, &temp);
  break;
  }
  case 'S': {
  BTWeapon temp (BT_SWAP); 
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  case 'M': {
  sendPlusMe (BT_WPN_LAUNCH, (*pimp_)[BT_MISSING]);
  break;
  }    
  case 'u': {
  BTWeapon temp (BT_UPBYSIDE); 
  sendPlusMe (BT_WPN_ON, &temp);
  break;
  }
  case '~': {
  short funds = 150;
  send (BT_FUNDS, &funds);
  }
  case 'b': {
  BTWeapon temp (BT_BOTTLE); 
  sendPlusMe (BT_WPN_ON, &temp);
  break;
  }
  case 't': {
  BTWeapon temp2 (BT_TWILIGHT);
  sendPlusMe (BT_WPN_ON, &temp2);
  break;
  }
  */
  case 'q': case 'Q': {
    cout << "\007You do not own BattleTris.  BattleTris owns you." << endl;
    return;
  }
/*  
  case 'w': {
  BTWeapon temp (BT_FEARED_WEIRD);
  sendPlusMe (BT_WPN_LAUNCH, &temp);
  break;
  }
  
  case 'W': {
  BTWeapon temp (BT_FEARED_WEIRD);
  sendPlusMe (BT_WPN_ON, &temp);
  break;
  }
  */
  }
  
E 5
I 5
	if (c >= '0' && c <= '9')
		weapon_manager_->launchWeapon ((int) (c - '0'));
	else if (c == 'q' || c == 'Q')
		cout << "\007You do not own BattleTris."
		     << "BattleTris owns you." << endl;
E 5
}

void BTGame::drop() {
D 4
  if (!current_piece_) {
    return;
  }
  if (!current_piece_->moveTo (x_, y_+delta_y_)) { 
    removeTimeOut (BT_SLICK_TIMEOUT);
    removeTimeOut (BT_DROP_TIMEOUT);
    sliding_ = 1;
E 4
I 4
	if (!current_piece_)
		return;

	if (!current_piece_->moveTo(x_, y_+delta_y_)) { 
		removeTimeOut(BT_SLICK_TIMEOUT);
		removeTimeOut(BT_DROP_TIMEOUT);
		sliding_ = 1;
E 4
    
D 4
    *slide_time_ = BT_SLIDE_TIME * (1 - weapon_manager_->BTActive[BT_NO_SLIDE]);
    addTimeOut (BT_SLIDE_TIMEOUT);
  } else y_ += delta_y_;
  
  DISPLAY->flush();
E 4
I 4
		*slide_time_ = BT_SLIDE_TIME *
		    (1 - weapon_manager_->BTActive[BT_NO_SLIDE]);
		addTimeOut(BT_SLIDE_TIMEOUT);
	} else {
		y_ += delta_y_;
D 6
		airslide_ = 0;
E 6
	}

	DISPLAY->flush();
E 4
}

void BTGame::place() {
D 4
  if (!current_piece_)
    return;
  if (!current_piece_->moveTo (x_, y_+delta_y_)) { 
    piece_manager_->dispose (current_piece_);
    current_piece_ = 0;
    board_->checkLines();
    score_manager_->update();
    board_->flushIdiot();    
    if (weapon_manager_->BTActive[BT_CONDOR]) {
      BTBoard temp (board_);
      temp.motivation(BT_CONDOR);
      comm_manager_->sendBoard (&temp);
    } else if (weapon_manager_->BTActive[BT_AMES]) {
      BTBoard temp (board_);
      temp.motivation(BT_AMES);
      comm_manager_->sendBoard (&temp);
    } else if (weapon_manager_->BTActive[BT_ACE]) {
      BTBoard temp (board_);
      temp.motivation(BT_ACE);
      comm_manager_->sendBoard (&temp);
    }
    comm_manager_->flushWeapons();
    x_ = def_x_; y_ = def_y_;
E 4
I 4
	if (!current_piece_)
		return;
D 6
	if (!current_piece_->moveTo(x_, y_+delta_y_)) { 
D 5
		if (airslide_ && *drop_time_ == fast_drop_time_) {
E 5
I 5
		if (airslide_ && drop_) {
E 6
I 6

	if (!current_piece_->moveTo(x_, y_ + delta_y_)) { 
		if (drop_ && sliding_ <= 1 &&
		    !current_piece_->canMoveTo(x_, y_ - delta_y_)) {
E 6
E 5
			/* we are go for airslide */
			send(BT_AIRSLIDE);
		}

		piece_manager_->dispose(current_piece_);
		current_piece_ = 0;
		board_->checkLines();
		score_manager_->update();
		board_->flushIdiot();    
		if (weapon_manager_->BTActive[BT_CONDOR]) {
			BTBoard temp(board_);
			temp.motivation(BT_CONDOR);
			comm_manager_->sendBoard (&temp);
		} else if (weapon_manager_->BTActive[BT_AMES]) {
			BTBoard temp (board_);
			temp.motivation(BT_AMES);
			comm_manager_->sendBoard (&temp);
		} else if (weapon_manager_->BTActive[BT_ACE]) {
			BTBoard temp (board_);
			temp.motivation(BT_ACE);
			comm_manager_->sendBoard(&temp);
		}

		comm_manager_->flushWeapons();
		x_ = def_x_;
		y_ = def_y_;
E 4
    
D 4
    current_piece_ = piece_manager_->create (def_x_, def_y_);
E 4
I 4
		current_piece_ = piece_manager_->create(def_x_, def_y_);
E 4
    
D 4
    x_ = def_x_ - current_piece_->rot_ / 2;
E 4
I 4
		x_ = def_x_ - current_piece_->rot_ / 2;
E 4
    
D 4
    if (!current_piece_->moveTo (x_, y_)) {
      sendPlusMe (BT_GAME_OVER);
      return;
    }
E 4
I 4
		if (!current_piece_->moveTo(x_, y_)) {
			sendPlusMe(BT_GAME_OVER);
			return;
		}
E 4
    
D 4
    if (weapon_manager_->BTActive[BT_SLICK]) 
      addTimeOut (BT_SLICK_TIMEOUT);
E 4
I 4
		if (weapon_manager_->BTActive[BT_SLICK]) 
			addTimeOut(BT_SLICK_TIMEOUT);
E 4
    
D 4
    if (chal_dialog_) {
      // We\'ve received a challenge....
      assert (computer_);
      if (!paused_) {
        pause();
      }
      chal_dialog_->show();
    }
E 4
I 4
		if (chal_dialog_) {
			// We\'ve received a challenge....
			assert(computer_);
			if (!paused_) {
				pause();
			}
			chal_dialog_->show();
		}
E 4
    
I 5
		drop_ = 0;
E 5
D 4
    *drop_time_ = base_drop_time_;
  } else 
    y_ += delta_y_;
  
  addTimeOut (BT_DROP_TIMEOUT);
  DISPLAY->flush();
E 4
I 4
		*drop_time_ = base_drop_time_;
	} else {
		y_ += delta_y_;
	}
D 6
	airslide_ = 0;
E 6

	addTimeOut (BT_DROP_TIMEOUT);
	DISPLAY->flush();
E 4
}

void BTGame::startGame() {
  XmProcessTraversal (*drawing_area_, XmTRAVERSE_CURRENT);
  stopwatch_.restart();
  
  current_piece_ = piece_manager_->create (def_x_, def_y_);

  x_ = def_x_ - current_piece_->rot_ / 2; y_ = 0;
  
  addTimeOut (BT_DROP_TIMEOUT);
}

void BTGame::endGame() {
  // Don\'t want to display a nasty message.
  send (BT_GAME_OVER, (void *) 1);
  cleanUp();
}

void BTGame::cleanUp() {  
  if (in_baz_ == 1)
    leaveBazaar();
  
  stopwatch_.stop();
  removeAllTimeOuts();

  sendPlusMe(BT_CONDOR_OFF, 0);
  
  stats_->winnerScore_ = score_manager_->rep_.op_score_;
  stats_->winnerLines_ = score_manager_->rep_.op_lines_;
  stats_->winnerFunds_ = score_manager_->max_op_funds_;
  stats_->loserScore_ = score_manager_->rep_.score_;
  stats_->loserLines_ = score_manager_->rep_.lines_;
  stats_->loserFunds_ = score_manager_->max_funds_;
  stats_->duration_ = stopwatch_.elapsed();
  if ( current_piece_ )
    current_piece_->reset();
  current_piece_ = 0;
  started_ = 0;

  if (condor_on_)
    condor();
  computer_ = 0;
}

void BTGame::reset() {
  m = 0;
  if ( current_piece_ )
    current_piece_->reset();
  current_piece_ = 0;
  chal_dialog_ = 0;
  swapper_ = 0;
  done_baz_ = 0;
  op_done_baz_ = 0;
  in_baz_ = -1;
  recon_->spy_on_ = 0;
  slick_dir_ = 0;

  def_x_ = BT_DEFAULT_X;
  def_y_ = BT_DEFAULT_Y;
  
  delta_y_ = 1;
  left_x_ = -1;
  right_x_ = 1;
  
  base_drop_time_ = BT_DROP_TIME;
  fast_drop_time_ = BT_FAST_DROP_TIME;
I 5

  drop_ = 0;
E 5
  *drop_time_ = base_drop_time_;

  unpauseAllTimeOuts();
  paused_ = 0;

  if ( initialized_ )
    board_->clear();
}
E 1
