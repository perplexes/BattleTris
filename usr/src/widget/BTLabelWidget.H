/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTLabelWidget.H                                     */
/*    ASGN: Final                                               */
/*    DATE: Thu Apr 21 14:06:43 1994                            */
/****************************************************************/

#ifndef BTLABELWIDGET_H
#define BTLABELWIDGET_H

#include "BTConfig.H"

#include <Xm/Label.h>

#include "BTWidget.H"

class BTLabelWidget : public BTWidget {
public:
	BTLabelWidget(BTWidget *parent, const char *name, const char *title);
	BTLabelWidget(BTWidget *parent, const char *name, const char *title,
	    Position x1, Position y1, Position x2, Position y2);
	BTLabelWidget(BTWidget *parent, const char *name, const char *title,
	    Dimension width, Dimension height);
	~BTLabelWidget()
	{
		if (input_struct_)
			delete input_struct_;
	}

	void setLabel(const char *new_label);

	void alignLeft() {
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_BEGINNING, NULL);
	}

	void alignCenter() {
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_CENTER, NULL);
	}
  
	void alignRight() {
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_END, NULL);
	}

	static void input_CB(Widget widget, XtPointer data,
	    XEvent *, Boolean *)
	{
		BTLabelWidget *t = (BTLabelWidget *) data;
		t->callback(t->input_struct_);
	}
	virtual void addInputCallback(void (*cb)(BTWidget *, void*),
	    void *data) { addCallback( input_struct_, cb, data ); }
	void setupStructs();
	ActivateCBStruct *input_struct_;
};

#endif
