/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTPushButtonWidget.H                                */
/*    ASGN: Images                                              */
/*    DATE: Tue Apr 19 20:04:17 1994                            */
/****************************************************************/

#ifndef BTPUSHBUTTONWIDGET_H
#define BTPUSHBUTTONWIDGET_H

#include "BTConfig.H"

#include <Xm/PushB.h>

#include "BTWidget.H"

class BTPushButtonWidget : public BTWidget {
public:
	BTPushButtonWidget(BTWidget *parent, const char *name,
	    const char *title);
	BTPushButtonWidget(BTWidget *parent, const char *name,
	    const char *title, Position x1, Position y1,
	    Position x2, Position y2);
	BTPushButtonWidget(BTWidget *parent, const char *name,
	    const char *title, Dimension width, Dimension height);
	~BTPushButtonWidget()
	{
		if (activate_struct_)
			delete activate_struct_;
	}

	void setLabel(const char *const new_label);

	static void
	activate_CB(Widget widget, XtPointer data, XtPointer cbs)
	{
		BTPushButtonWidget *t = (BTPushButtonWidget *)data;
		t->callback(t->activate_struct_);
	}

	virtual void
	addActivateCallback(void (*cb)(BTWidget *, void *), void *data)
	{
		addCallback( activate_struct_, cb, data );
	}

	void
	alignLeft()
	{
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_BEGINNING, NULL);
	}

	void
	alignCenter()
	{
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_CENTER, NULL);
	}
  
	void
	alignRight()
	{
		XtVaSetValues(me_, XmNalignment, XmALIGNMENT_END, NULL);
	}

protected:
	ActivateCBStruct *activate_struct_;
};

#endif
