h38392
s 00035/00033/00043
d D 1.2 01/10/23 00:05:26 bmc 3 1
c 1000017 Ernie needs levels other than "Hard" and "Impossible"
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:25 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTChallenge.H
c Name history : 1 0 src/game/BTChallenge.H
e
s 00076/00000/00000
d D 1.1 01/10/20 13:35:24 bmc 1 0
c date and time created 01/10/20 13:35:24 by bmc
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
/*    FILE: BTChallenge.H                                       */
/*    DATE: Wed Apr 20 17:51:19 1994                            */
/****************************************************************/

#ifndef BTCHALLENGE_HEADER
#define BTCHALLENGE_HEADER

#include "BTConfig.H"

#include <X11/cursorfont.h>

#include "BTWidget.H"
#include "BTScrolledTextWidget.H"
#include "BTScrolledListWidget.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"
#include "BTFrameWidget.H"
#include "BTRowColumnWidget.H"
#include "BTCheckBoxWidget.H"
#include "BTDrawingAreaWidget.H"
I 3
#include "BTLabelWidget.H"
#include "BTSliderWidget.H"
E 3

#define BT_CHALLENGE_WIDTH 640
#define BT_CHALLENGE_HEIGHT 600

class BTNetManager;
class BTPixmap;

class BTChallenge {
D 3

E 3
friend class BTStartup;
D 3

E 3
public:
D 3
  BTChallenge(BTWidget *parent, BTNetManager *netMgr, BTPixmap *);
  ~BTChallenge() {}
E 3
I 3
	BTChallenge(BTWidget *parent, BTNetManager *netMgr, BTPixmap *);
	~BTChallenge() {}
E 3

D 3
  void show();
  void hide();
E 3
I 3
	void show();
	void hide();
E 3

D 3
  BTWIDGET_CALLBACK(BTChallenge, handleSelection)
  BTWIDGET_CALLBACK(BTChallenge, handleUpdate)
  BTWIDGET_CALLBACK(BTChallenge, handleChallenge)
  BTWIDGET_CALLBACK(BTChallenge, handleToggle)
  BTWIDGET_CALLBACK(BTChallenge, handleSuperErnie)
E 3
I 3
	BTWIDGET_CALLBACK(BTChallenge, handleSelection)
	BTWIDGET_CALLBACK(BTChallenge, handleUpdate)
	BTWIDGET_CALLBACK(BTChallenge, handleChallenge)
	BTWIDGET_CALLBACK(BTChallenge, handleToggle)
	BTWIDGET_CALLBACK(BTChallenge, handleLevel)
E 3

protected:
D 3
  BTNetManager *netMgr_;
  int selection_;
  int avail_, super_ernie_;
  Cursor cursor_;
E 3
I 3
	BTNetManager *netMgr_;
	int selection_;
	int avail_, level_;
	Cursor cursor_;
E 3

D 3
  // Do not change the order of the widget declarations since they
  // need to be constructed in the right order in the initializer list
  // of BTChallenge::BTChallenge
E 3
I 3
	/*
	 * Do not change the order of the widget declarations since they
	 * need to be constructed in the right order in the initializer list
	 * of BTChallenge::BTChallenge.  C++ is the best!
	 */
	BTFormWidget form_;
	BTScrolledTextWidget user_info_;
	BTScrolledListWidget user_list_;
	BTPushButtonWidget challenge_button_;
	BTPushButtonWidget update_button_;
	BTPushButtonWidget cancel_button_;
	BTFrameWidget computer_frame_;
	BTLabelWidget computer_label_;
	BTFormWidget computer_form_;
	BTPushButtonWidget computer_button_;
	BTSliderWidget ernie_slider_;
	BTCheckBoxWidget computer_toggle_;
	BTDrawingAreaWidget drawing_area_;
E 3

D 3
  BTFormWidget form_;
  BTScrolledTextWidget user_info_;
  BTScrolledListWidget user_list_;
  BTPushButtonWidget challenge_button_;
  BTPushButtonWidget update_button_;
  BTPushButtonWidget cancel_button_;
  BTFrameWidget computer_frame_;
  BTRowColumnWidget computer_rowcol_;
  BTPushButtonWidget computer_button_;
  BTCheckBoxWidget computer_toggle_, super_ernie_toggle_;
  BTDrawingAreaWidget drawing_area_;

E 3
protected:
D 3
  static void _chaltimeout_ (void *, unsigned long *);
  void update();
E 3
I 3
	static void _chaltimeout_ (void *, unsigned long *);
	void update();
E 3
};

D 3

E 3
#endif
E 1
