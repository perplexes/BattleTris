/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTScrolledTextWidget.H                              */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:20:24 1994                            */
/****************************************************************/

#ifndef BTSCROLLEDTEXTWIDGET_H
#define BTSCROLLEDTEXTWIDGET_H

#include "BTConfig.H"

#include <Xm/Text.h>

#include "BTWidget.H"

#define BTSCROLLEDTEXTWIDGET_DEFWIDTH	44

class BTScrolledTextWidget : public BTWidget {
protected:
	Widget text_w_;

public:
	BTScrolledTextWidget(BTWidget *parent, const char *name,
	    const char *text);
	BTScrolledTextWidget(BTWidget *parent, const char *name,
	    const char *text, Position x1, Position y1,
	    Position x2, Position y2);

	~BTScrolledTextWidget()
	{
		XtDestroyWidget(text_w_);
		me_ = NULL;
	}

	void splitLines(char *text, int width);
	void setText(const char *text, Boolean split = True,
	    int width = BTSCROLLEDTEXTWIDGET_DEFWIDTH);

	Widget getTextWidget() { return text_w_; }
};

#endif
