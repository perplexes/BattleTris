/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTTextWidget.H                                      */
/*    ASSN:                                                     */
/*    DATE: Thu Apr 28 23:12:10 1994                            */
/****************************************************************/

#ifndef BTTEXTWIDGET_H
#define BTTEXTWIDGET_H

#include "BTConfig.H"

#include <Xm/Text.h>

#include "BTWidget.H"

class BTTextWidget : public BTWidget {
protected:
	short rows_, cols_;
	short tab_;

public:
	BTTextWidget(BTWidget *parent, const char *name,
	    Position x1, Position y1, Position x2, Position y2,
	    short rows = 25, short cols = 80);
	~BTTextWidget() {}

	void setTab(short tab) { tab_ = tab; }
	void setText(short row, short tab_stop, const char *text);
};

#endif
