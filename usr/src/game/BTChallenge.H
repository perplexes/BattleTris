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
#include "BTLabelWidget.H"
#include "BTSliderWidget.H"

#define BT_CHALLENGE_WIDTH 640
#define BT_CHALLENGE_HEIGHT 600

class BTNetManager;
class BTPixmap;

class BTChallenge {
friend class BTStartup;
public:
	BTChallenge(BTWidget *parent, BTNetManager *netMgr, BTPixmap *);
	~BTChallenge() {}

	void show();
	void hide();

	BTWIDGET_CALLBACK(BTChallenge, handleSelection)
	BTWIDGET_CALLBACK(BTChallenge, handleUpdate)
	BTWIDGET_CALLBACK(BTChallenge, handleChallenge)
	BTWIDGET_CALLBACK(BTChallenge, handleToggle)
	BTWIDGET_CALLBACK(BTChallenge, handleLevel)

protected:
	BTNetManager *netMgr_;
	int selection_;
	int avail_, level_;
	Cursor cursor_;

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

protected:
	static void _chaltimeout_ (void *, unsigned long *);
	void update();
};

#endif
