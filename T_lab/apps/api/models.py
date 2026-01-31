from sqlalchemy import Column, String, Float, JSON, DateTime, Integer, Boolean, Text, ForeignKey, create_engine
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import sessionmaker, relationship
from datetime import datetime
import os

Base = declarative_base()

class SessionModel(Base):
    __tablename__ = "sessions"

    session_id = Column(String, primary_key=True)
    created_at = Column(DateTime, default=datetime.now)
    user_input = Column(Text, nullable=False)
    domain = Column(String)
    status = Column(String)
    intent = Column(String)
    intent_confidence = Column(Float)
    
    # Simulatability check
    simulatable = Column(Boolean, default=True)
    simulation_reason = Column(String)
    alternative_method = Column(String)
    
    novelty_score = Column(Float)
    novelty_reasoning = Column(Text)
    feasibility_grade = Column(String)
    
    # Store complex structures as JSON
    proposed_methods = Column(JSON)
    selected_method = Column(JSON)
    selected_method_index = Column(Integer)
    
    # Missing fields added
    literature_context = Column(JSON)
    search_queries = Column(JSON)
    simulation_params = Column(JSON)
    execution_logs = Column(JSON)
    error = Column(String)
    
    experiment_code = Column(Text)
    experiment_results = Column(JSON)
    simulation_results = Column(JSON)
    
    draft_report = Column(Text)
    final_report = Column(Text)
    report_path = Column(String)
    
    critique = Column(Text)
    improvements = Column(JSON)
    quality_score = Column(Float)
    
    verified_citations = Column(JSON)
    unverified_citations = Column(JSON)
    
    current_step = Column(String)
    current_step_label = Column(String)
    
    # Logic chain and activity logs
    logic_chain = Column(JSON)
    activity_log = Column(JSON)

# SQLite connection for local file-based storage (No Docker required)
DATABASE_URL = "sqlite:///./t_lab.db"

# connect_args={"check_same_thread": False} is needed for SQLite with FastAPI
engine = create_engine(
    DATABASE_URL, 
    connect_args={"check_same_thread": False}
)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

def init_db():
    Base.metadata.create_all(bind=engine)
