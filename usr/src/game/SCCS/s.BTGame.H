h04430
s 00000/00001/00162
d D 1.5 01/10/22 22:30:59 ahl 6 5
c 1000013 props for airslides (I suck)
e
s 00001/00000/00162
d D 1.4 01/10/22 21:11:05 ahl 5 4
c 1000013 props for airslides (lint)
e
s 00001/00000/00161
d D 1.3 01/10/22 17:56:46 ahl 4 3
c 1000013 props for airslides
e
s 00081/00089/00080
d D 1.2 01/10/21 19:25:09 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:28 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTGame.H
c Name history : 1 0 src/game/BTGame.H
e
s 00169/00000/00000
d D 1.1 01/10/20 13:35:27 bmc 1 0
c date and time created 01/10/20 13:35:27 by bmc
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
/*    FILE: BTGame.H                                            */
/*    ASSN:                                                     */
/*    DATE: Mon Feb 21 18:30:41 1994                            */
/****************************************************************/

#ifndef BTGAME_H
#define BTGAME_H

#include "BTConfig.H"

#define BT_GAME_WIDTH 670
#define BT_GAME_HEIGHT 700
#define BT_GAME_SCORE_X 300
#define BT_GAME_SCORE_Y 30
#define BT_GAME_ARSENAL_X 300
#define BT_GAME_ARSENAL_Y 270

#define BT_GAME_FRAC_BASE 800

#include <Xm/MainW.h>
#include <Xm/DrawingA.h>

#include "BTConstants.H"
#include "BTProtocol.H"

#include <BTScrolledTextWidget.H>
#include <BTScrolledListWidget.H>
#include <BTPushButtonWidget.H>
#include <BTFormWidget.H>
#include <BTLabelWidget.H>
#include <BTDrawingAreaWidget.H>

#include "BTRingNode.H"
#include "BTTimeOut.H"

#include <stdlib.h>

#define BT_DROP_TIMEOUT         0
#define BT_SLIDE_TIMEOUT        1
#define BT_SLICK_TIMEOUT        2
#define BT_HATTER_TIMEOUT       3
#define BT_JEOPARDY_TIMEOUT     4
#define BT_MAX_GAME_TIMEOUT     5

#define XK_LATIN1
#include <X11/keysymdef.h>
#include "BTStopwatch.H"

class BTBazaar;
class BTPimp;
class BTPieceManager;
class BTScoreManager;
class BTSoundManager;
class BTCommManager;
class BTWeaponManager;
class BTBoardManager;
class BTRecon;
class BTStopwatch;
class BTGameStats;
class BTComputer;
class BTChallengeDialog;
class BTPixmap;
class BTPiece;

class BTGame : public BTRingNode {
friend class BTStartup;
friend class BTCommManager;
D 3

E 3
private:
I 3
	BTTimeOut timeout_[BT_MAX_GAME_TIMEOUT];
E 3

D 3
  BTTimeOut timeout_[BT_MAX_GAME_TIMEOUT];
E 3
I 3
	int sliding_;
I 4
D 6
	int airslide_;
E 6
I 5
	int drop_;
E 5
E 4
	BTGameStats *stats_;
	BTPimp *pimp_;
	BTFormWidget *form_;
	BTDrawingAreaWidget *drawing_area_;
	BTWidget *parent_;
	int slick_dir_;
	int swapper_, paused_, started_;
	int op_done_baz_, done_baz_, in_baz_;
	int condor_on_;
	int base_drop_time_, fast_drop_time_;
	int *drop_time_, *slide_time_;
	int x_, y_;
	int def_x_, def_y_;
	int delta_y_;
	int left_x_, right_x_;
	char opponent_msg_[255];
	unsigned short old_width_, old_height_;
	short m;
	BTRecon *recon_;
	BTBoardManager *board_, *initial_board_;
	BTPieceManager *piece_manager_;
	BTScoreManager *score_manager_;
	BTSoundManager *sound_manager_;
	BTCommManager *comm_manager_;
	BTWeaponManager *weapon_manager_;
	BTStopwatch stopwatch_;
	BTLabelWidget *message_;
	BTBazaar *bazaar_;
	BTPiece *current_piece_;
	BTComputer *computer_;
	BTChallengeDialog *chal_dialog_;
	static void _keypress2_(char, void *);
	static void _pausekey2_(char, void *);
E 3

D 3
  int sliding_;
  BTGameStats *stats_;
  BTPimp *pimp_;
  BTFormWidget *form_;
  BTDrawingAreaWidget *drawing_area_;
  BTWidget *parent_;
  int slick_dir_;
  int swapper_, paused_, started_;
  int op_done_baz_, done_baz_, in_baz_;
  int condor_on_;
  int base_drop_time_, fast_drop_time_;
  int *drop_time_, *slide_time_;
  int x_, y_;
  int def_x_, def_y_;
  int delta_y_;
  int left_x_, right_x_;
  char opponent_msg_[255];
  unsigned short old_width_, old_height_;
  short m;
  BTRecon *recon_;
  BTBoardManager *board_, *initial_board_;
  BTPieceManager *piece_manager_;
  BTScoreManager *score_manager_;
  BTSoundManager *sound_manager_;
  BTCommManager *comm_manager_;
  BTWeaponManager *weapon_manager_;
  BTStopwatch stopwatch_;
  BTLabelWidget *message_;
  BTBazaar *bazaar_;
  BTPiece *current_piece_;
  BTComputer *computer_;
  BTChallengeDialog *chal_dialog_;
  static void _keypress2_( char, void * );
  static void _pausekey2_( char, void * );
//  static void _gamecb_ (Widget, XtPointer, XtPointer);
//  static void _keypress_ (Widget, XtPointer, XEvent *, Boolean *);
//  static void _pausekey_ (Widget, XtPointer, XEvent *, Boolean *);
//  static void _bazcb_ (Widget, XtPointer thisp, XtPointer cd);
E 3
I 3
	BT_TIMEOUT_CALLBACK (BTGame, timeout);
	BT_TIMEOUT_CALLBACK (BTGame, slidetime);
	BT_TIMEOUT_CALLBACK (BTGame, hattertime);
	BT_TIMEOUT_CALLBACK (BTGame, slicktime);
	BT_TIMEOUT_CALLBACK (BTGame, jeotime);
E 3

D 3
  BT_TIMEOUT_CALLBACK (BTGame, timeout);
  BT_TIMEOUT_CALLBACK (BTGame, slidetime);
  BT_TIMEOUT_CALLBACK (BTGame, hattertime);
  BT_TIMEOUT_CALLBACK (BTGame, slicktime);
  BT_TIMEOUT_CALLBACK (BTGame, jeotime);
E 3
I 3
	void exposeEvent ();
	void leaveBazaar();
	void runComputer();
E 3

D 3
  void exposeEvent ();
  void leaveBazaar();
  void runComputer();
E 3
I 3
	void addTimeOut (int id);
	void removeTimeOut (int id);
	void pauseTimeOut (int id);
	void unpauseTimeOut (int id);
	void expireTimeOut (int id);
	void clearTimeOut (int id);
	void removeAllTimeOuts();
	void pauseAllTimeOuts();
	void unpauseAllTimeOuts();
E 3

D 3
  void addTimeOut (int id);
  void removeTimeOut (int id);
  void pauseTimeOut (int id);
  void unpauseTimeOut (int id);
  void expireTimeOut (int id);
  void clearTimeOut (int id);
  void removeAllTimeOuts();
  void pauseAllTimeOuts();
  void unpauseAllTimeOuts();
E 3
I 3
	BTPixmap *gimp_pixmap_;
E 3

D 3
  BTPixmap *gimp_pixmap_;

E 3
public:
D 3
  BTGame (BTWidget *parent, BTSoundManager *sound_manager, BTCommManager *, 
	  BTPimp *pimp, BTBazaar *bazaar, BTPixmap *gimp = NULL );
  ~BTGame();
  void receive(BTRingPacket *packet);
  void startGame();
  void endGame();
  void pause(int no_send = 0);
  void unpause();
  void keyPressed (char c);
  void drop();
  void place();
  void beginDrop();
  void moveLeft();
  void moveRight();
  void condor();
  void reset();
  void cleanUp();
  void rotate();
  void challenge(BTChallengeDialog *chal ) {
    chal_dialog_ = chal; }
  void run() {
    sendPlusMe (BT_START, "error.");
  }
  int gimpMe( int, int );
  int isDropping() { return *drop_time_ == fast_drop_time_; }
  static void _gamecb2_(BTWidget *, void *d) {
    ((BTGame *)d)->exposeEvent();
  }
  static void _bazcb2_(BTWidget *, void *);
E 3
I 3
	BTGame(BTWidget *parent, BTSoundManager *sound_manager,
	    BTCommManager *, BTPimp *pimp, BTBazaar *bazaar,
	    BTPixmap *gimp = NULL);
	~BTGame();
	void receive(BTRingPacket *packet);
	void startGame();
	void endGame();
	void pause(int no_send = 0);
	void unpause();
	void keyPressed (char c);
	void drop();
	void place();
	void beginDrop();
	void moveLeft();
	void moveRight();
	void condor();
	void reset();
	void cleanUp();
	void rotate();
	void challenge(BTChallengeDialog *chal ) { chal_dialog_ = chal; }
	void run() { sendPlusMe (BT_START, (void *)"error."); }
	int gimpMe( int, int );
	int isDropping() { return *drop_time_ == fast_drop_time_; }
	static void _gamecb2_(BTWidget *, void *d) {
		((BTGame *)d)->exposeEvent();
	}
	static void _bazcb2_(BTWidget *, void *);
E 3
};    
D 3
      
E 3
I 3

E 3
#endif
E 1
