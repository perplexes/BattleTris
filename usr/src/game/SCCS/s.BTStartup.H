h42652
s 00002/00000/00143
d D 1.2 01/10/23 20:14:54 bmc 3 1
c 1000020 Need better handling of image loading failure
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:33 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTStartup.H
c Name history : 1 0 src/game/BTStartup.H
e
s 00143/00000/00000
d D 1.1 01/10/20 13:35:32 bmc 1 0
c date and time created 01/10/20 13:35:32 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTStartup.H                                         */
/*    DATE: Wed Apr 20 17:51:19 1994                            */
/****************************************************************/

#ifndef BTSTARTUP_H
#define BTSTARTUP_H

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include <stdio.h>
#include <fcntl.h>
#include <iostream.h>

#include <Xm/Xm.h>
#include <Xm/DialogS.h>
#include <Xm/MessageB.h>

#include "BTTimeOut.H"
#include "BTChallenge.H"
#include "BTAbout.H"
#include "BTChallengeDialog.H"
#include "BTRoster.H"
#include "BTBiff.H"
#include "BTBazaar.H"

#define BT_STARTUP_WIDTH 640
#define BT_STARTUP_HEIGHT 600

enum location_t {
  BT_STARTUP,  // Main screen
  BT_BIFF,     // Sleeping
  BT_CHAL,     // Challenge window
  BT_ABOUT,    // About box
  BT_ROSTER,   // Roster window
  BT_ERNIE     // Temple of Doom
};

class BTComputer;
class BTWidget;
class BTPushButtonWidget;
class BTFormWidget;
class BTLabelWidget;
class BTDrawingAreaWidget;
class BTCommManager;
class BTNetManager;
class BTSoundManager;
class BTPlayer;
class DevAudio;
class PPMReader;
class BTBiff;
class BTChallenge;
class BTAbout;
class BTChallengeDialog;
class BTRoster;
class BTGame;
class BTBazaar;
class BTPimp;
class BTPixmap;

class BTStartup {

public:
  BTStartup( BTWidget * );
  ~BTStartup();

//  void challenge(char *challenger, char *node);
  void challenge( BTPlayer * );
  void won() { won_ = 1; }
  void error() { won_ = -1; }
  int accepted() { return accepted_; }

  void show(int first_time = 0);
  void hide();

  void showGame();
  void hideGame();

  void childDied();

  BT_TIMEOUT_CALLBACK(BTStartup, challengeTimeOut);
  BT_TIMEOUT_CALLBACK(BTStartup, gameOverTimeOut);

  BTWIDGET_CALLBACK(BTStartup, handleChallenge)
  BTWIDGET_CALLBACK(BTStartup, handleSleep)
  BTWIDGET_CALLBACK(BTStartup, handleQuit)

  BTWIDGET_CALLBACK(BTStartup, handleBiffClick)       
  BTWIDGET_CALLBACK(BTStartup, handleYes)       
  BTWIDGET_CALLBACK(BTStartup, handleNo)

  BTWIDGET_CALLBACK(BTStartup, handleComputer)
  BTWIDGET_CALLBACK(BTStartup, handleAbout)
  BTWIDGET_CALLBACK(BTStartup, handleAboutOK)       

  BTWIDGET_CALLBACK(BTStartup, handleChalCancel)

  BTWIDGET_CALLBACK(BTStartup, handleRoster)       
  BTWIDGET_CALLBACK(BTStartup, handleRosterDone)
     
  XtAppContext app_;
  BTWidget *parent_;

protected:
  BTNetManager *net_manager_;
  BTCommManager *comm_manager_,*computer_comm_;
  BTBazaar *bazaar_;
  BTAbout *about_box_;
  BTChallenge *challenge_screen_;
  BTChallengeDialog *chal_dialog_;
  BTRoster *roster_;
  BTGame *game_;
  BTPimp *pimp_;
  PPMReader *reader_;
  BTComputer *computer_;
  BTSoundManager *sound_manager_;
  DevAudio *dev_;
  location_t location_;
  Widget chal_rep_;
  BTFormWidget *form_;
  BTBiff *biff_;
  int biff_click_, challenge_, accepted_, challengee_, won_, playing_again_;
  short chal_timeouts_, rejections_;
  Position x_, y_;
  XtIntervalId chal_time_;
  BTPushButtonWidget *challenge_button_;
  BTPushButtonWidget *sleep_button_;
  BTPushButtonWidget *about_button_;
  BTPushButtonWidget *roster_button_;
  BTPushButtonWidget *quit_button_;
  BTDrawingAreaWidget *drawing_area_;
  char mainid_[50], iconid_[50];
  BTPixmap *gimp_image_;
  BTPlayer *challenger_;
I 3

	BTPixmap *loadImage(const char *name);
E 3
};

#endif
E 1
