h63515
s 00034/00053/00031
d D 1.2 01/10/21 19:25:16 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:15 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTPushButtonWidget.C
c Name history : 1 0 src/widget/BTPushButtonWidget.C
e
s 00084/00000/00000
d D 1.1 01/10/20 13:35:14 bmc 1 0
c date and time created 01/10/20 13:35:14 by bmc
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
/*    FILE: BTPushButtonWidget.C                                */
/*    ASGN: Images                                              */
/*    DATE: Tue Apr 19 20:09:00 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTXmUtils.H"
#include "BTPushButtonWidget.H"

D 3
BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, char *const name,
                                       char *const title)
E 3
I 3
BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, const char *name,
    const char *title)
E 3
: BTWidget(parent), activate_struct_(0) 
{
D 3
  XmString str = xm_strcreate(title);
  me_ = XtVaCreateWidget(name, xmPushButtonWidgetClass,
                         parent ? parent->getWidget() : 0,
                         XmNlabelString, str, NULL);
  XmStringFree(str);
  XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer) this);
E 3
I 3
	XmString str = xm_strcreate(title);
	me_ = XtVaCreateWidget(name, xmPushButtonWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str, NULL);
	XmStringFree(str);
	XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer)this);
E 3
}

D 3
BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, char *const name,
                                       char *const title, Position x1,
                                       Position y1, Position x2, Position y2)
E 3
I 3
BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, const char *name,
    const char *title, Position x1, Position y1, Position x2, Position y2)
E 3
: BTWidget(parent), activate_struct_(0) 
{
D 3
  XmString str = XmStringCreateLtoR(title, XmFONTLIST_DEFAULT_TAG);
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
E 3

D 3
  me_ = XtVaCreateWidget(name, xmPushButtonWidgetClass,
                         parent ? parent->getWidget() : (Widget) NULL,
                         XmNlabelString, str, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmPushButtonWidgetClass,
	    parent ? parent->getWidget() : (Widget) NULL,
	    XmNlabelString, str, NULL);
E 3

D 3
  size( x1, y1, x2-x1, y2-y1 );
/*
  
		     XmNx, 
                     XmNtopAttachment, XmATTACH_POSITION,
                     XmNtopPosition, y1,
                     XmNleftAttachment, XmATTACH_POSITION,
                     XmNleftPosition, x1,
                     XmNrightAttachment, XmATTACH_POSITION,
                     XmNrightPosition, x2,
                     XmNbottomAttachment, XmATTACH_POSITION,
                     XmNbottomPosition, y2,
//                     XmNresizePolicy, XmRESIZE_NONE,
		     NULL);
		     */
  XmStringFree(str);
E 3
I 3
	size(x1, y1, x2 - x1, y2 - y1);
	XmStringFree(str);
E 3

D 3
  XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer) this);

E 3
I 3
	XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer)this);
E 3
}

D 3
BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, char *const name,
				       char *const title,
				       Dimension width, Dimension height)
E 3
I 3
BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, const char *name,
    const char *title, Dimension width, Dimension height)
E 3
: BTWidget(parent), activate_struct_(0) 
{
D 3
  XmString str = XmStringCreateLtoR(title, XmFONTLIST_DEFAULT_TAG);
  me_  = XtVaCreateWidget(name, xmPushButtonWidgetClass,
                          parent ? parent->getWidget() : 0,
			  XmNlabelString, str, NULL);
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_  = XtVaCreateWidget(name, xmPushButtonWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str, NULL);
E 3

D 3
  size((Dimension) -1, (Dimension) -1, width, height );
/*			  XmNwidth, width, XmNheight, height,
			  XmNresizePolicy, XmRESIZE_NONE,
                          XmNrecomputeSize, False,
			  NULL);
			  */
  XmStringFree(str);
E 3
I 3
	size((Dimension)-1, (Dimension)-1, width, height );
	XmStringFree(str);
E 3

D 3
  XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer) this);

E 3
I 3
	XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer)this);
E 3
}

D 3
void BTPushButtonWidget::setLabel(char *const new_label)
E 3
I 3
void
BTPushButtonWidget::setLabel(const char *new_label)
E 3
{
D 3
  XmString str = XmStringCreateLtoR(new_label, XmFONTLIST_DEFAULT_TAG);
  XtVaSetValues(me_, XmNlabelString, str, NULL);
  XmStringFree(str);
E 3
I 3
	XmString str =
	    XmStringCreateLtoR((char *)new_label, XmFONTLIST_DEFAULT_TAG);
	XtVaSetValues(me_, XmNlabelString, str, NULL);
	XmStringFree(str);
E 3
}
E 1
